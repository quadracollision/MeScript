(ns glitchlisp.swing.render
  (:require [clojure.java.io :as io]
            [clojure.set]
            [glitchlisp.swing.editor :as editor]
            [glitchlisp.swing.shared :as shared])
  (:import
    [java.io ByteArrayInputStream ByteArrayOutputStream File PushbackReader StringReader]
    [javax.sound.sampled AudioFileFormat$Type AudioFormat AudioInputStream AudioSystem Clip]
    [javax.swing JComboBox JFileChooser JFrame JLabel JOptionPane SwingUtilities]
    [javax.swing.text JTextComponent]))

(def resource-slurp shared/resource-slurp)
(def app-dir shared/app-dir)
(def child-file shared/child-file)
(def state shared/state)
(def set-status! shared/set-status!)
(def default-audio-device-label shared/default-audio-device-label)
(def set-combo-items! shared/set-combo-items!)
(def focus-source-error! editor/focus-source-error!)
(def report-source-error! editor/report-source-error!)
(def validate-delimiters! editor/validate-delimiters!)

(defn choose-file [parent mode]
  (let [chooser (JFileChooser. ".")]
    (when (= JFileChooser/APPROVE_OPTION (.showDialog chooser parent mode))
      (.getSelectedFile chooser))))

(defn read-file [^File file]
  (slurp (.getPath file)))

(defn write-file! [^File file text]
  (when-let [parent (.getParentFile file)]
    (.mkdirs parent))
  (spit (.getPath file) text))

(declare current-file-or-session!)

(defn include-path-from-line [line]
  (when-let [[_ path] (re-matches #"\s*\(include\s+\"([^\"]+)\"\)\s*" line)]
    path))

(defn canonical-file [^File file]
  (try
    (.getCanonicalFile file)
    (catch Exception _
      (.getAbsoluteFile file))))

(defn expand-source-includes
  ([source source-file]
   (expand-source-includes source source-file #{}))
  ([source source-file seen]
   (let [base (or (some-> ^File source-file .getParentFile)
                  (File. "."))]
     (apply str
            (mapcat
              (fn [line]
                (if-let [include-path (include-path-from-line line)]
                  (let [file (File. include-path)
                        file (if (.isAbsolute file) file (File. base include-path))
                        canonical (canonical-file file)]
                    (when (contains? seen canonical)
                      (throw (ex-info (str "include cycle detected at " (.getPath file))
                                      {:file file})))
                    (let [included (slurp (.getPath file))]
                      [(expand-source-includes included file (conj seen canonical))
                       (when-not (clojure.string/ends-with? included "\n")
                         "\n")]))
                  [line "\n"]))
              (clojure.string/split-lines source))))))

(defn compile-glitchlisp-source [source]
  (if (.exists (File. "src/compiler.clj"))
    (load-file "src/compiler.clj")
    (when-let [compiler-source (resource-slurp "compiler.clj")]
      (load-string compiler-source)))
  (let [source (expand-source-includes source (current-file-or-session!))
        compiler (ns-resolve 'glitchlisp-compiler 'compile-source)]
    (if compiler
      (compiler source)
      source)))

(defn current-file-or-session! []
  (or (:file @state)
      (File. "mescript-swing-session.gl")))

(defn stop-clip! []
  (when-let [^Clip clip (:clip @state)]
    (when (.isRunning clip)
      (.stop clip))
    (.close clip))
  (swap! state assoc :clip nil))

(defn play-wav! [^File wav loop?]
  (stop-clip!)
  (let [stream (AudioSystem/getAudioInputStream wav)
        clip (AudioSystem/getClip)]
    (.open clip stream)
    (swap! state assoc :clip clip)
    (if loop?
      (.loop clip Clip/LOOP_CONTINUOUSLY)
      (.start clip))))

(defn run-command! [args]
  (let [builder (ProcessBuilder. ^java.util.List args)
        _ (.redirectErrorStream builder true)
        process (.start builder)
        output (slurp (.getInputStream process))
        code (.waitFor process)]
    (when-not (zero? code)
      (throw (ex-info output {:exit code})))
    output))

(declare ensure-renderer!)

(defn refresh-audio-devices! [^JComboBox combo ^JLabel status]
  (future
    (try
      (let [renderer (ensure-renderer! status)
            output (run-command! [renderer "devices-plain"])
            devices (->> (clojure.string/split-lines output)
                         (map clojure.string/trim)
                         (remove clojure.string/blank?)
                         distinct)
            values (into [default-audio-device-label] devices)]
        (SwingUtilities/invokeLater
          #(do
             (set-combo-items! combo values)
             (set-status! status (str "audio devices: " (count devices))))))
      (catch Exception ex
        (SwingUtilities/invokeLater
          #(set-status! status (str "audio device refresh failed: " (.getMessage ex))))))))

(defn audio-devices! [status]
  (let [renderer (ensure-renderer! status)
        output (run-command! [renderer "devices-plain"])]
    (->> (clojure.string/split-lines output)
         (map clojure.string/trim)
         (remove clojure.string/blank?)
         distinct
         vec)))

(defn renderer-path []
  (let [exe (if (clojure.string/includes? (System/getProperty "os.name") "Windows")
              "glitchlisp-native.exe"
              "glitchlisp-native")
        release (File. (str "target/release/" exe))
        debug (File. (str "target/debug/" exe))
        local (File. exe)
        app-local (child-file (app-dir) exe)]
    (cond
      (.exists release) (.getPath release)
      (.exists debug) (.getPath debug)
      (.exists local) (.getPath local)
      (.exists app-local) (.getPath app-local)
      :else nil)))

(def expected-renderer-capabilities
  #{"null-params" "empty-gate-silent" "gui-live" "live-audio-info" "check-live-source"
    "gate-then-times" "scene-loop-true" "scene-loop-by" "sample-form" "gui-render-preview" "drum-note-pitch"
    "native-compiler-source" "native-compile-command" "drunk"})

(defn parse-renderer-capabilities [output]
  (->> (clojure.string/split output #"\s+")
       (remove clojure.string/blank?)
       set))

(defn renderer-capabilities [renderer]
  (try
    (parse-renderer-capabilities (run-command! [renderer "capabilities"]))
    (catch Exception _
      #{})))

(defn renderer-compatible? [renderer]
  (clojure.set/subset? expected-renderer-capabilities
                       (renderer-capabilities renderer)))

(defn compatible-renderer-path []
  (when-let [renderer (renderer-path)]
    (when (renderer-compatible? renderer)
      renderer)))

(defn ensure-renderer! [status]
  (or (compatible-renderer-path)
      (do
        (SwingUtilities/invokeLater #(set-status! status "building renderer..."))
        (run-command! ["cargo" "build" "--release"])
        (or (compatible-renderer-path)
            (throw (ex-info "cargo build finished, but compatible target/release/glitchlisp-native was not found"
                            {:exit 1
                             :required expected-renderer-capabilities
                             :renderer (renderer-path)}))))))

(defn playback-command-line? [line depth]
  (and (zero? depth)
       (boolean (re-matches #"\s*\((start!|play-scene|play-block|cue)(\s+.*)?\)\s*" line))))

(defn scan-depth-after-line [state line]
  (let [text (str line "\n")]
    (loop [idx 0
           {:keys [depth in-string? escape? in-comment?]} state]
      (if (>= idx (count text))
        {:depth depth
         :in-string? in-string?
         :escape? escape?
         :in-comment? in-comment?}
        (let [ch (.charAt text idx)]
          (cond
            in-comment?
            (recur (inc idx)
                   {:depth depth
                    :in-string? in-string?
                    :escape? false
                    :in-comment? (not= ch \newline)})

            escape?
            (recur (inc idx)
                   {:depth depth
                    :in-string? in-string?
                    :escape? false
                    :in-comment? false})

            in-string?
            (recur (inc idx)
                   {:depth depth
                    :in-string? (not= ch \")
                    :escape? (= ch \\)
                    :in-comment? false})

            (= ch \;)
            (recur (inc idx)
                   {:depth depth
                    :in-string? false
                    :escape? false
                    :in-comment? true})

            (= ch \")
            (recur (inc idx)
                   {:depth depth
                    :in-string? true
                    :escape? false
                    :in-comment? false})

            (or (= ch \() (= ch \[) (= ch \{))
            (recur (inc idx)
                   {:depth (inc depth)
                    :in-string? false
                    :escape? false
                    :in-comment? false})

            (or (= ch \)) (= ch \]) (= ch \}))
            (recur (inc idx)
                   {:depth (max 0 (dec depth))
                    :in-string? false
                    :escape? false
                    :in-comment? false})

            :else
            (recur (inc idx)
                   {:depth depth
                    :in-string? false
                    :escape? false
                    :in-comment? false})))))))

(defn strip-playback-commands [source]
  (let [lines (clojure.string/split-lines source)]
    (loop [remaining lines
           state {:depth 0 :in-string? false :escape? false :in-comment? false}
           kept []]
      (if-let [line (first remaining)]
        (let [remove? (playback-command-line? line (:depth state))
              next-state (scan-depth-after-line state line)]
          (recur (rest remaining)
                 next-state
                 (if remove? kept (conj kept line))))
        (->> kept
             (clojure.string/join "\n")
             clojure.string/trim)))))

(defn source-with-cue [source scene]
  (str (strip-playback-commands source) "\n\n(play-scene :" scene ")\n"))

(declare read-source-forms)

(defn has-play-command? [source]
  (try
    (boolean
      (some #(and (seq? %)
                  (contains? #{'start! 'play-scene 'play-block 'cue} (first %)))
            (read-source-forms source)))
    (catch Exception _
      false)))
(defn first-scene-name [source]
  (try
    (some #(when (and (seq? %)
                      (contains? #{'scene 'block} (first %)))
              (some-> (second %) name))
          (read-source-forms source))
    (catch Exception _
      nil)))

(defn top-level-playable-form? [form]
  (and (seq? form)
       (contains? #{'d 'sample} (first form))))

(defn has-track-form? [source]
  (try
    (boolean (some top-level-playable-form? (read-source-forms source)))
    (catch Exception _
      false)))

(defn preview-source [source]
  (cond
    (has-play-command? source) source
    (first-scene-name source) (source-with-cue source (first-scene-name source))
    (has-track-form? source) (str (clojure.string/trim source) "\n\n(start!)\n")
    :else source))

(defn require-playback-form! [source]
  (when-not (has-play-command? source)
    (throw (ex-info "add an explicit playback form: (play-scene :scene-name) for scenes or (start!) for top-level tracks"
                    {}))))

(defn wav-file-for-name [name]
  (let [trimmed (clojure.string/trim name)
        named (if (clojure.string/blank? trimmed) "swing-preview.wav" trimmed)
        wav-name (if (clojure.string/ends-with? (clojure.string/lower-case named) ".wav")
                   named
                   (str named ".wav"))
        file (File. wav-name)]
    (if (or (.isAbsolute file)
            (clojure.string/includes? wav-name "/")
            (clojure.string/includes? wav-name "\\"))
      file
      (File. "renders" wav-name))))

(defn bpm-from-source [source]
  (try
    (let [bpms (keep #(when (and (seq? %)
                                 (= 'bpm (first %))
                                 (number? (second %)))
                        (double (second %)))
                     (read-source-forms source))]
      (if (seq bpms)
        (last bpms)
        124.0))
    (catch Exception _
      124.0)))

(defn seconds-for-steps [source steps]
  (/ steps (/ (* (bpm-from-source source) 4.0) 60.0)))

(defn gate-hold-token? [token]
  (boolean (re-matches #"1_(?:[0-9]+)?" token)))

(defn gate-hold-replacement [token]
  (let [amount-text (subs token 2)
        amount (if (clojure.string/blank? amount-text) "1" amount-text)]
    (str "(gate-hold " amount ")")))

(defn rewrite-gate-hold-tokens [source]
  (let [token-end? #(or (Character/isWhitespace ^char %)
                        (contains? #{\( \) \[ \] \{ \} \; \,} %))]
    (loop [idx 0
           out []
           in-string? false
           escape? false
           in-comment? false]
      (if (< idx (count source))
        (let [ch (.charAt source idx)]
          (cond
            in-comment?
            (recur (inc idx) (conj out ch) in-string? false (not= ch \newline))

            escape?
            (recur (inc idx) (conj out ch) in-string? false false)

            (and in-string? (= ch \\))
            (recur (inc idx) (conj out ch) true true false)

            (= ch \")
            (recur (inc idx) (conj out ch) (not in-string?) false false)

            in-string?
            (recur (inc idx) (conj out ch) true false false)

            (= ch \;)
            (recur (inc idx) (conj out ch) false false true)

            (Character/isDigit ch)
            (let [end (loop [cursor idx]
                        (if (and (< cursor (count source))
                                 (not (token-end? (.charAt source cursor))))
                          (recur (inc cursor))
                          cursor))
                  token (subs source idx end)]
              (if (gate-hold-token? token)
                (recur end (conj out (gate-hold-replacement token)) false false false)
                (recur end (conj out token) false false false)))

            :else
            (recur (inc idx) (conj out ch) false false false)))
        (apply str out)))))

(defn read-source-forms [source]
  (binding [*read-eval* false]
    (with-open [reader (PushbackReader. (StringReader. (rewrite-gate-hold-tokens source)))]
      (loop [forms []]
        (let [form (read reader false ::eof)]
          (if (= form ::eof)
            forms
            (recur (conj forms form))))))))

(defn form-head [form]
  (when (seq? form)
    (first form)))

(defn pair-value [items key]
  (loop [remaining items]
    (when (seq remaining)
      (if (= key (first remaining))
        (second remaining)
        (recur (nnext remaining))))))

(def missing-option ::missing-option)

(defn pair-value-or [items key fallback]
  (loop [remaining items]
    (if (seq remaining)
      (if (= key (first remaining))
        (second remaining)
        (recur (nnext remaining)))
      fallback)))

(defn positive-runtime-int-value [value name]
  (cond
    (not (number? value))
    (throw (ex-info (str name " must be numeric") {:value value}))

    (or (neg? value) (not (integer? value)))
    (throw (ex-info (str name " must be a non-negative integer") {:value value}))

    (zero? value)
    (throw (ex-info (str name " must be greater than zero") {:value value}))

    :else
    (int value)))

(defn non-negative-runtime-int-value [value name]
  (cond
    (not (number? value))
    (throw (ex-info (str name " must be numeric") {:value value}))

    (or (neg? value) (not (integer? value)))
    (throw (ex-info (str name " must be a non-negative integer") {:value value}))

    :else
    (int value)))

(defn positive-times-count [value]
  (if (number? value)
    (let [n (int value)]
      (if (pos? n)
        n
        (throw (ex-info "times must be greater than zero" {:value value}))))
    (throw (ex-info "times must be a number" {:value value}))))

(defn truthy-value? [value]
  (cond
    (= value false) false
    (= value 'false) false
    (= value 0) false
    (nil? value) false
    :else true))

(defn loop-true-option! [value]
  (if (truthy-value? value)
    0
    (throw (ex-info "scene :loop only accepts true; use :repeat N for finite scenes"
                    {:value value}))))

(defn gcd-int [a b]
  (loop [a (Math/abs (long a))
         b (Math/abs (long b))]
    (if (zero? b)
      (max 1 a)
      (recur b (mod a b)))))

(defn lcm-int
  ([a b]
   (let [a (max 1 (long a))
         b (max 1 (long b))]
     (* (quot a (gcd-int a b)) b)))
  ([values]
   (reduce lcm-int 1 values)))

(declare gate-step-bools gate-pattern-summary note-pattern-summary)

(defn truthy-gate? [value]
  (if (number? value)
    (not (zero? value))
    (throw (ex-info "expected numeric pattern value" {:value value}))))

(defn expand-gate-cell [pattern width]
  (let [pattern (vec pattern)
        width (max 1 width)
        scale (quot width (max 1 (count pattern)))
        expanded (vec (repeat width false))]
    (if (empty? pattern)
      expanded
      (reduce (fn [result [idx gate]]
                (if gate
                  (assoc result (* idx scale) true)
                  result))
              expanded
              (map-indexed vector pattern)))))

(defn validate-gate-hold! [expr]
  (let [amount (second expr)]
    (when (> (count expr) 2)
      (throw (ex-info "gate-hold expects zero or one amount" {:form expr})))
    (when amount
      (let [hold (non-negative-runtime-int-value amount "gate-hold")]
        (when (zero? hold)
          (throw (ex-info "gate-hold must be greater than zero" {:form expr})))))))

(defn gate-step-bools [expr]
  (if (vector? expr)
    (if (empty? expr)
      [false]
      (let [children (mapv gate-step-bools expr)
            cell-width (lcm-int (map #(max 1 (count %)) children))]
        (vec (mapcat #(expand-gate-cell % cell-width) children))))
    (if (and (seq? expr) (= 'gate-hold (form-head expr)))
      (do
        (validate-gate-hold! expr)
        [true])
      [(truthy-gate? expr)])))

(defn euclid-bools
  ([pulses steps rotation]
   (euclid-bools pulses steps rotation "euclid"))
  ([pulses steps rotation form-name]
   (let [steps (positive-runtime-int-value steps (str form-name " steps"))
         pulses (non-negative-runtime-int-value pulses (str form-name " pulses"))
         rotation (mod (non-negative-runtime-int-value rotation (str form-name " rotation")) steps)]
    (mapv (fn [idx]
            (let [rotated (mod (+ idx steps (- rotation)) steps)]
              (< (mod (* rotated pulses) steps) pulses)))
          (range steps)))))

(defn gate-summary-from-steps [steps]
  (let [expanded (if (seq steps) (mapv gate-step-bools steps) [[false]])
        slots (reduce + (map count expanded))
        hits (reduce + (map #(count (filter true? %)) expanded))]
    {:length (max 1 (count expanded))
     :hits hits
     :slots (max 1 slots)}))

(defn scale-gate-summary [summary n]
  (let [n (positive-times-count n)]
    {:length (max 1 (* n (:length summary)))
     :hits (* n (:hits summary))
     :slots (max 1 (* n (:slots summary)))}))

(defn combine-gate-summaries [summaries]
  (let [summaries (seq summaries)]
    (if summaries
      {:length (reduce + (map :length summaries))
       :hits (reduce + (map :hits summaries))
       :slots (max 1 (reduce + (map :slots summaries)))}
      {:length 1 :hits 0 :slots 1})))

(defn require-pattern-source! [expr form-name]
  (or (second expr)
      (throw (ex-info (str form-name " requires a pattern") {:form expr}))))

(defn validate-reverse-form! [expr]
  (when (> (count expr) 2)
    (throw (ex-info "reverse expects one pattern" {:form expr})))
  (or (second expr)
      (throw (ex-info "reverse requires a pattern" {:form expr}))))

(defn validate-note-vector-form! [expr form-name]
  (when (> (count expr) 2)
    (throw (ex-info (str form-name " expects one vector") {:form expr})))
  (let [source (second expr)]
    (when-not (vector? source)
      (throw (ex-info (str form-name " requires a vector") {:form expr})))
    source))

(defn note-symbol? [value]
  (and (symbol? value)
       (boolean (re-matches #"(?i)[a-g](s|b)?-?\d+" (name value)))))

(defn validate-note-cell! [value]
  (cond
    (number? value) nil
    (note-symbol? value) nil
    (symbol? value) (throw (ex-info (str "unknown symbol '" value "'") {:value value}))
    :else (throw (ex-info "expected number or note" {:value value}))))

(defn validate-note-pattern-vector! [values]
  (doseq [value values]
    (if (vector? value)
      (doseq [note value]
        (validate-note-cell! note))
      (validate-note-cell! value))))

(defn gate-pattern-summary [expr]
  (cond
    (and (seq? expr) (= 'euclid (form-head expr)))
    (let [[_ pulses steps] expr]
      (when (not= 3 (count expr))
        (throw (ex-info "euclid expects pulses and steps" {:form expr})))
      (gate-summary-from-steps (mapv #(if % 1 0) (euclid-bools pulses steps 0))))

    (and (seq? expr) (= 'euclid-rot (form-head expr)))
    (let [[_ pulses steps rotation] expr]
      (when (not= 4 (count expr))
        (throw (ex-info "euclid-rot expects pulses, steps, and rotation" {:form expr})))
      (gate-summary-from-steps
       (mapv #(if % 1 0)
             (euclid-bools pulses steps rotation "euclid-rot"))))

    (and (seq? expr) (contains? #{'rev 'reverse} (form-head expr)))
    (gate-pattern-summary (validate-reverse-form! expr))

    (and (seq? expr) (= 'times (form-head expr)))
    (let [n (or (second expr)
                (throw (ex-info "times requires a count" {:form expr})))
          pattern (or (nth expr 2 nil)
                      (throw (ex-info "times requires a pattern" {:form expr})))]
      (when (> (count expr) 3)
        (throw (ex-info "times expects count and one pattern" {:form expr})))
      (scale-gate-summary (gate-pattern-summary pattern) n))

    (and (seq? expr) (= 'then (form-head expr)))
    (do
      (when (< (count expr) 3)
        (throw (ex-info "then expects at least two patterns" {:form expr})))
      (combine-gate-summaries (map gate-pattern-summary (rest expr))))

    (and (seq? expr) (= 'p (form-head expr)))
    (do
      (let [source (require-pattern-source! expr "p")]
        (when (> (count expr) 2)
          (if (some #(and (symbol? %) (= 'then %)) (drop 2 expr))
            (throw (ex-info "p wraps exactly one pattern; use (p (then A B)) instead of (p A then B)"
                            {:form expr}))
            (throw (ex-info "p expects one pattern" {:form expr}))))
        (if (vector? source)
          (gate-summary-from-steps source)
          (gate-pattern-summary source))))

    (vector? expr)
    (gate-summary-from-steps expr)

    :else
    (gate-summary-from-steps [expr])))

(defn note-pattern-summary [expr]
  (cond
    (and (seq? expr) (contains? #{'rev 'reverse} (form-head expr)))
    (note-pattern-summary (validate-reverse-form! expr))

    (and (seq? expr) (= 'p (form-head expr)))
    (let [source (validate-note-vector-form! expr "p")]
      (validate-note-pattern-vector! source)
      {:mode :step :length (max 1 (count source))})

    (and (seq? expr) (= 's (form-head expr)))
    (let [source (validate-note-vector-form! expr "s")]
      (validate-note-pattern-vector! source)
      {:mode :hit :length (max 1 (count source))})

    (and (seq? expr)
         (contains? #{'g 'gs 'gate-seq 'gate_seq} (form-head expr)))
    (let [form-name (name (form-head expr))
          source (validate-note-vector-form! expr form-name)]
      (validate-note-pattern-vector! source)
      {:mode :tick :length (max 1 (count source))})

    (vector? expr)
    (do
      (validate-note-pattern-vector! expr)
      {:mode :hit :length (max 1 (count expr))})

    :else
    (do
      (validate-note-cell! expr)
      {:mode :step :length 1})))

(defn track-param-items [track-form]
  (case (form-head track-form)
    sample (if (keyword? (nth track-form 2 nil))
             (drop 2 track-form)
             (drop 3 track-form))
    (drop 2 track-form)))

(defn sample-inline-source-option? [value]
  (contains? #{:sample-data :sample_data :sample :sample-path :sample_path} value))

(defn null-symbol? [value]
  (and (symbol? value)
       (contains? #{"nil" "null"} (name value))))

(defn validate-sample-header! [track-form]
  (when (= 'sample (form-head track-form))
    (when-not (keyword? (second track-form))
      (throw (ex-info "sample track id must be a keyword" {:form track-form})))
    (let [source (nth track-form 2 missing-option)
          options-start (if (keyword? source) 2 3)
          options (drop options-start track-form)]
      (when (= missing-option source)
        (throw (ex-info "sample requires a wav path or :sample-data" {:form track-form})))
      (when (and (not (keyword? source))
                 (not (string? source))
                 (not (null-symbol? source)))
        (throw (ex-info "expected string" {:value source})))
      (loop [remaining options]
        (when (seq remaining)
          (let [key (first remaining)]
            (when-not (keyword? key)
              (throw (ex-info "sample options must be keyword/value pairs" {:form track-form})))
            (when-not (next remaining)
              (throw (ex-info (str "sample " key " requires a value") {:option key})))
            (recur (nnext remaining)))))
      (when (and (keyword? source)
                 (not (some #(and (keyword? %) (sample-inline-source-option? %))
                            (take-nth 2 options))))
        (throw (ex-info "sample requires a wav path or :sample-data" {:form track-form}))))))

(defn validate-sample-data-value! [value]
  (when-not (vector? value)
    (throw (ex-info "sample-data must be a vector" {:value value})))
  (when (empty? value)
    (throw (ex-info "sample-data requires at least one value" {:value value})))
  (doseq [cell value]
    (validate-note-cell! cell)))

(defn validate-sample-path-value! [value]
  (when-not (or (string? value) (null-symbol? value))
    (throw (ex-info "expected string" {:value value}))))

(def valid-source-names
  #{"sine-synth" "saw-synth" "square-synth" "tri-synth"
    "pulse" "pulse-synth" "morph" "morph-synth"
    "supersaw" "supersaw-synth" "wavetable" "wavetable-synth"
    "fm-op" "fm_op" "fm-op-synth" "additive" "additive-synth"
    "sync" "sync-synth" "pwm-sweep" "pwm_sweep" "harsh" "chip"
    "pluck" "strings" "brass" "organ" "bell" "glass" "vocal" "breath"
    "pad-wash" "pad_wash" "click" "noise-synth"
    "kick-synth" "snare" "snare-synth" "hat" "hat-synth"
    "kick-808" "808-kick" "snare-808" "808-snare" "hat-808" "808-hat"
    "cowbell-808" "808-cowbell"
    "kick-909" "909-kick" "snare-909" "909-snare" "hat-909" "909-hat"
    "kick-78" "78-kick" "snare-78" "78-snare" "hat-78" "78-hat"
    "kick-707" "707-kick" "snare-707" "707-snare"
    "clap" "cymbal-crash" "cymbal_crash" "cymbal-ride" "cymbal_ride"
    "tom" "rimshot" "shaker" "woodblock" "cowbell" "zap" "scratch"
    "impact" "bass-slap" "bass_slap" "piano-electric" "piano_electric"
    "drone-dark" "drone_dark" "noise-white" "noise_white"
    "noise-pink" "noise_pink" "noise-brown" "noise_brown"
    "noise-blue" "noise_blue" "noise-purple" "noise_purple" "sample"})

(defn validate-source-value! [value]
  (when-not (null-symbol? value)
    (let [source-name (cond
                        (keyword? value) (name value)
                        (symbol? value) (name value)
                        :else (throw (ex-info "source must be a keyword" {:value value})))]
      (when-not (contains? valid-source-names source-name)
        (throw (ex-info (str "unsupported source ':" source-name "'") {:value value}))))))

(defn trim-float [value]
  (let [text (str (double value))]
    (if (clojure.string/ends-with? text ".0")
      (subs text 0 (- (count text) 2))
      text)))

(defn numeric-only! [value]
  (if (number? value)
    (double value)
    (throw (ex-info "expected numeric pattern value" {:value value}))))

(defn validate-harmonics-value! [value]
  (let [number (numeric-only! value)]
    (when-not (<= 0.0 number 2.0)
      (throw (ex-info (str "harmonics must be between 0 and 2, got " (trim-float number))
                      {:value value})))))

(defn validate-harmonics! [value]
  (when-not (vector? value)
    (throw (ex-info "harmonics must be a vector" {:value value})))
  (when (> (count value) 8)
    (throw (ex-info (str "harmonics accepts at most 8 values, got " (count value))
                    {:value value})))
  (doseq [cell value]
    (validate-harmonics-value! cell)))

(defn validate-fx-value! [value]
  (when-not (null-symbol? value)
    (cond
      (vector? value)
      (doseq [effect value]
        (when-not (seq? effect)
          (throw (ex-info "effect must be a form" {:value effect}))))

      (seq? value) nil

      :else
      (throw (ex-info "fx must be a vector of effect forms" {:value value})))))

(defn bounded-number! [value min-value max-value name]
  (let [number (numeric-only! value)]
    (when-not (<= min-value number max-value)
      (throw (ex-info (str name " must be between "
                           (trim-float min-value) " and "
                           (trim-float max-value) ", got "
                           (trim-float number))
                      {:value value})))
    number))

(defn min-number! [value min-value name]
  (let [number (numeric-only! value)]
    (when (< number min-value)
      (throw (ex-info (str name " must be at least "
                           (trim-float min-value) ", got "
                           (trim-float number))
                      {:value value})))
    number))

(defn non-negative-integer-number! [value name]
  (let [number (numeric-only! value)]
    (when (or (neg? number)
              (not (zero? (rem number 1.0))))
      (throw (ex-info (str name " must be a non-negative integer") {:value value})))
    (int number)))

(defn bounded-integer! [value min-value max-value name]
  (let [number (non-negative-integer-number! value name)]
    (when-not (<= min-value number max-value)
      (throw (ex-info (str name " must be between "
                           min-value " and " max-value ", got " number)
                      {:value value})))
    number))

(defn validate-numeric-pattern-vector! [value min-value max-value name]
  (doseq [cell value]
    (bounded-number! cell min-value max-value name)))

(defn validate-min-numeric-pattern-vector! [value min-value name]
  (doseq [cell value]
    (min-number! cell min-value name)))

(defn validate-integer-pattern-vector! [value min-value max-value name]
  (doseq [cell value]
    (bounded-integer! cell min-value max-value name)))

(defn validate-bounded-track-number! [value min-value max-value name]
  (when-not (null-symbol? value)
    (cond
      (and (seq? value) (contains? #{'p 'g 'gs 'gate-seq 'gate_seq} (form-head value)))
      (let [source (validate-note-vector-form! value (str (form-head value)))]
        (validate-numeric-pattern-vector! source min-value max-value name))

      (and (seq? value) (contains? #{'rev 'reverse} (form-head value)))
      (validate-bounded-track-number! (validate-reverse-form! value) min-value max-value name)

      (vector? value)
      (validate-numeric-pattern-vector! value min-value max-value name)

      :else
      (bounded-number! value min-value max-value name))))

(defn validate-min-track-number! [value min-value name]
  (when-not (null-symbol? value)
    (cond
      (and (seq? value) (contains? #{'p 'g 'gs 'gate-seq 'gate_seq} (form-head value)))
      (let [source (validate-note-vector-form! value (str (form-head value)))]
        (validate-min-numeric-pattern-vector! source min-value name))

      (and (seq? value) (contains? #{'rev 'reverse} (form-head value)))
      (validate-min-track-number! (validate-reverse-form! value) min-value name)

      (vector? value)
      (validate-min-numeric-pattern-vector! value min-value name)

      :else
      (min-number! value min-value name))))

(defn validate-bounded-integer-track-number! [value min-value max-value name]
  (when-not (null-symbol? value)
    (cond
      (and (seq? value) (contains? #{'p 'g 'gs 'gate-seq 'gate_seq} (form-head value)))
      (let [source (validate-note-vector-form! value (str (form-head value)))]
        (validate-integer-pattern-vector! source min-value max-value name))

      (and (seq? value) (contains? #{'rev 'reverse} (form-head value)))
      (validate-bounded-integer-track-number! (validate-reverse-form! value) min-value max-value name)

      (vector? value)
      (validate-integer-pattern-vector! value min-value max-value name)

      :else
      (bounded-integer! value min-value max-value name))))

(defn validate-bounded-track-param! [value min-value max-value key name]
  (try
    (validate-bounded-track-number! value min-value max-value name)
    (catch Exception ex
      (throw (ex-info (str key " " (.getMessage ex))
                      (ex-data ex)
                      ex)))))

(defn validate-min-track-param! [value min-value key name]
  (try
    (validate-min-track-number! value min-value name)
    (catch Exception ex
      (throw (ex-info (str key " " (.getMessage ex))
                      (ex-data ex)
                      ex)))))

(defn validate-bounded-integer-track-param! [value min-value max-value key name]
  (try
    (validate-bounded-integer-track-number! value min-value max-value name)
    (catch Exception ex
      (throw (ex-info (str key " " (.getMessage ex))
                      (ex-data ex)
                      ex)))))

(defn validate-f32-track-number! [value]
  (when-not (null-symbol? value)
    (cond
      (and (seq? value) (contains? #{'p 'g 'gs 'gate-seq 'gate_seq} (form-head value)))
      (validate-numeric-pattern-vector! (validate-note-vector-form! value (str (form-head value)))
                                        Double/NEGATIVE_INFINITY
                                        Double/POSITIVE_INFINITY
                                        "")

      (and (seq? value) (contains? #{'rev 'reverse} (form-head value)))
      (validate-f32-track-number! (validate-reverse-form! value))

      (vector? value)
      (validate-numeric-pattern-vector! value
                                        Double/NEGATIVE_INFINITY
                                        Double/POSITIVE_INFINITY
                                        "")

      :else
      (validate-note-cell! value))))

(defn validate-f32-track-param! [value key]
  (try
    (validate-f32-track-number! value)
    (catch Exception ex
      (throw (ex-info (str key " " (.getMessage ex))
                      (ex-data ex)
                      ex)))))

(defn validate-non-negative-integer-param! [value key name]
  (try
    (when-not (null-symbol? value)
      (non-negative-integer-number! value name))
    (catch Exception ex
      (throw (ex-info (.getMessage ex)
                      (ex-data ex)
                      ex)))))

(def track-param-keys
  #{:src :note :gate :detune :detune-cents :phase :pulse-width :pulse_width :pw
    :morph :morph-pos :morph_pos :gain :unison :unison-detune :unison_detune
    :unison-spread :unison_spread :spread :fm-ratio :fm_ratio :fm-depth :fm_depth
    :harmonics :sample :sample-path :sample_path :sample-data :sample_data
    :every :offset :drunk :amp :dur :fx})

(defn track-param-canonical-key [key]
  (case key
    (:detune :detune-cents) :detune
    (:pulse-width :pulse_width :pw) :pulse-width
    (:morph :morph-pos :morph_pos) :morph
    (:unison-detune :unison_detune) :unison-detune
    (:unison-spread :unison_spread :spread) :unison-spread
    (:fm-ratio :fm_ratio) :fm-ratio
    (:fm-depth :fm_depth) :fm-depth
    (:sample :sample-path :sample_path :sample-data :sample_data) :sample
    key))

(defn validate-track-params! [track-form]
  (loop [remaining (track-param-items track-form)
         seen #{}]
    (when (seq remaining)
      (let [key (first remaining)]
        (when-not (keyword? key)
          (throw (ex-info "track parameters must be keyword/value pairs" {:form track-form})))
        (when-not (contains? track-param-keys key)
          (throw (ex-info (str "unknown track parameter '" key "'") {:option key})))
        (when-not (next remaining)
          (throw (ex-info (str "track parameter '" key "' requires a value") {:option key})))
        (when (= :src key)
          (validate-source-value! (second remaining)))
        (when (contains? #{:sample-data :sample_data} key)
          (validate-sample-data-value! (second remaining)))
        (when (contains? #{:sample :sample-path :sample_path} key)
          (validate-sample-path-value! (second remaining)))
        (when (= :harmonics key)
          (validate-harmonics! (second remaining)))
        (when (= :fx key)
          (validate-fx-value! (second remaining)))
        (when (= :amp key)
          (validate-bounded-track-param! (second remaining) 0.0 1.0 key "amp"))
        (when (= :dur key)
          (validate-bounded-track-param! (second remaining) 0.005 4.0 key "dur"))
        (when (contains? #{:detune :detune-cents} key)
          (validate-f32-track-param! (second remaining) key))
        (when (= :phase key)
          (validate-f32-track-param! (second remaining) key))
        (when (contains? #{:pulse-width :pulse_width :pw} key)
          (validate-bounded-track-param! (second remaining) 0.01 0.99 key "pulse-width"))
        (when (contains? #{:morph :morph-pos :morph_pos} key)
          (validate-bounded-track-param! (second remaining) 0.0 1.0 key "morph"))
        (when (= :gain key)
          (validate-bounded-track-param! (second remaining) 0.0 2.0 key "gain"))
        (when (= :unison key)
          (validate-bounded-integer-track-param! (second remaining) 1 10 key "unison"))
        (when (= :offset key)
          (validate-non-negative-integer-param! (second remaining) key "offset"))
        (when (= :drunk key)
          (validate-bounded-track-param! (second remaining) 0.0 100.0 key "drunk"))
        (when (contains? #{:unison-detune :unison_detune} key)
          (validate-bounded-track-param! (second remaining) 0.0 100.0 key "unison-detune"))
        (when (contains? #{:unison-spread :unison_spread :spread} key)
          (validate-bounded-track-param! (second remaining) 0.0 1.0 key "unison-spread"))
        (when (contains? #{:fm-ratio :fm_ratio} key)
          (validate-min-track-param! (second remaining) 0.01 key "fm-ratio"))
        (when (contains? #{:fm-depth :fm_depth} key)
          (validate-bounded-track-param! (second remaining) 0.0 32.0 key "fm-depth"))
        (let [canonical (track-param-canonical-key key)]
          (when (contains? seen canonical)
            (throw (ex-info (str "duplicate track parameter '" key "'") {:option key})))
          (recur (nnext remaining) (conj seen canonical)))))))

(defn track-loop-steps [track-form]
  (validate-sample-header! track-form)
  (validate-track-params! track-form)
  (let [items (track-param-items track-form)
        gate (or (pair-value items :gate) 1)
        note (or (pair-value items :note) 'c3)
        every-value (pair-value-or items :every missing-option)
        every (if (= missing-option every-value)
                1
                (positive-runtime-int-value every-value "every"))
        {:keys [length hits slots]} (gate-pattern-summary gate)
        note-summary (note-pattern-summary note)
        note-length (:length note-summary)
        note-period (case (:mode note-summary)
                      :step note-length
                      :hit (if (zero? hits)
                             length
                             (* length (quot note-length (gcd-int note-length hits))))
                      :tick (* length (quot note-length (gcd-int note-length slots)))
                      note-length)]
    (* every (lcm-int length note-period))))

(defn top-level-track? [form]
  (and (seq? form) (contains? #{'d 'sample} (form-head form))))

(defn scene-form? [form]
  (and (seq? form) (contains? #{'scene 'block} (form-head form))))

(defn scene-name [form]
  (when (scene-form? form)
    (second form)))

(def scene-option-keys
  #{:repeat :repeats :times :loop :steps :length :bars :bar-steps :bar-length :bar-steps-of :bar-length-of :steps-of :length-of :loop-by :next})

(defn scene-option-canonical-key [key]
  (case key
    (:repeat :repeats :times :loop) :repeat
    (:steps :length :bars :steps-of :length-of :loop-by) :steps
    (:bar-steps :bar-length :bar-steps-of :bar-length-of) :bar-steps
    key))

(defn scene-option-missing-value-message [key]
  (case key
    (:repeat :repeats :times) "scene :repeat requires a value"
    :loop "scene :loop requires a value"
    (:steps :length) "scene :steps requires a value"
    :bars "scene :bars requires a value"
    (:bar-steps :bar-length) "scene :bar-steps requires a value"
    (:bar-steps-of :bar-length-of) "scene :bar-steps-of requires a keyword argument"
    (:steps-of :length-of) "scene :steps-of requires a keyword argument"
    :loop-by "scene :loop-by requires a track keyword"
    :next "scene :next requires a keyword argument"
    (str "scene option '" key "' requires a value")))

(defn validate-scene-option-value! [key value]
  (when (nil? value)
    (throw (ex-info (scene-option-missing-value-message key) {:option key})))
  (when (and (contains? #{:steps-of :length-of :bar-steps-of :bar-length-of :loop-by :next} key)
             (not (keyword? value)))
    (throw (ex-info (case key
                      (:steps-of :length-of) "scene :steps-of requires a keyword argument"
                      (:bar-steps-of :bar-length-of) "scene :bar-steps-of requires a keyword argument"
                      :loop-by "scene :loop-by requires a track keyword"
                      :next "scene :next requires a keyword argument")
                    {:option key :value value}))))

(defn scene-option-arity [key]
  (if (= key :loop-by) 3 2))

(def missing-scene-option ::missing-scene-option)

(defn scene-option-value-or [form target-key fallback]
  (loop [remaining (drop 2 form)]
    (if (seq remaining)
      (let [key (first remaining)]
        (cond
          (= target-key key)
          (second remaining)

          (keyword? key)
          (recur (drop (scene-option-arity key) remaining))

          :else
          fallback))
      fallback)))

(defn scene-option-value [form key]
  (let [value (scene-option-value-or form key missing-scene-option)]
    (when-not (= value missing-scene-option)
      value)))

(defn validate-scene-options! [form]
  (loop [remaining (drop 2 form)
         seen #{}]
    (when (seq remaining)
      (if (keyword? (first remaining))
        (let [key (first remaining)
              value (second remaining)]
          (when-not (contains? scene-option-keys key)
            (throw (ex-info (str "unknown scene option '" key "'") {:option key})))
          (validate-scene-option-value! key value)
          (when (= key :loop-by)
            (let [count-value (nth remaining 2 nil)]
              (when (nil? count-value)
                (throw (ex-info "scene :loop-by requires a count" {:option key})))
              (positive-runtime-int-value count-value "loop-by")))
          (let [canonical (scene-option-canonical-key key)]
            (when (contains? seen canonical)
              (throw (ex-info (str "duplicate scene option '" key "'") {:option key})))
            (recur (drop (scene-option-arity key) remaining) (conj seen canonical))))
        nil))))

(defn scene-body-forms [scene-form]
  (loop [remaining (drop 2 scene-form)]
    (cond
      (empty? remaining) []
      (keyword? (first remaining)) (recur (drop (scene-option-arity (first remaining)) remaining))
      :else remaining)))

(defn track-id [track-form]
  (when (top-level-track? track-form)
    (second track-form)))

(defn scene-inferred-steps [form]
  (validate-scene-options! form)
  (let [body (scene-body-forms form)
        tracks (filter top-level-track? body)]
    (if-let [target (or (scene-option-value form :steps-of)
                        (scene-option-value form :length-of))]
      (if-let [track (some #(when (= target (track-id %)) %) tracks)]
        (track-loop-steps track)
        (throw (ex-info (str "scene :steps-of references unknown track '" target "'")
                        {:track target})))
      (if (seq tracks)
        (lcm-int (map track-loop-steps tracks))
        (throw (ex-info "scene has nothing to play; add a track or set :steps explicitly"
                        {:form form}))))))

(defn scene-loop-by-steps [form]
  (when-let [target (scene-option-value form :loop-by)]
    (let [count-value (nth (drop-while #(not= :loop-by %) form) 2 nil)
          count (positive-runtime-int-value count-value "loop-by")]
      (if-let [track (some #(when (= target (track-id %)) %)
                           (filter top-level-track? (scene-body-forms form)))]
        (* count (track-loop-steps track))
        (throw (ex-info (str "scene :loop-by references unknown track '" target "'")
                        {:track target}))))))

(defn scene-steps-from-form [form]
  (validate-scene-options! form)
  (cond
    (scene-option-value form :steps)
    (positive-runtime-int-value (scene-option-value form :steps) "steps")

    (scene-option-value form :length)
    (positive-runtime-int-value (scene-option-value form :length) "steps")

    (scene-option-value form :loop-by)
    (scene-loop-by-steps form)

    (scene-option-value form :bars)
    (let [bar-count (positive-runtime-int-value (scene-option-value form :bars) "bars")
          explicit-bar-steps (or (scene-option-value form :bar-steps)
                                 (scene-option-value form :bar-length))
          bar-steps-target (or (scene-option-value form :bar-steps-of)
                               (scene-option-value form :bar-length-of))
          per-bar (cond
                    explicit-bar-steps
                    (positive-runtime-int-value explicit-bar-steps "bar-steps")

                    bar-steps-target
                    (if-let [track (some #(when (= bar-steps-target (track-id %)) %)
                                         (filter top-level-track? (scene-body-forms form)))]
                      (track-loop-steps track)
                      (throw (ex-info (str "scene :bar-steps-of references unknown track '" bar-steps-target "'")
                                      {:track bar-steps-target})))

                    :else
                    16)]
      (* bar-count per-bar))

    (or (scene-option-value form :bar-steps)
        (scene-option-value form :bar-length)
        (scene-option-value form :bar-steps-of)
        (scene-option-value form :bar-length-of))
    (throw (ex-info "scene :bar-steps requires :bars" {:form form}))

    :else
    (scene-inferred-steps form)))

(defn scene-repeat-from-form [form]
  (validate-scene-options! form)
  (let [loop-value (scene-option-value-or form :loop missing-scene-option)]
    (if (not= missing-scene-option loop-value)
      (loop-true-option! loop-value)
      (let [repeat-value (or (scene-option-value form :repeat)
                             (scene-option-value form :repeats)
                             (scene-option-value form :times))]
        (if (nil? repeat-value)
          (if (and (scene-option-value form :loop-by)
                   (scene-option-value form :next))
            1
            0)
          (non-negative-runtime-int-value repeat-value "repeat"))))))

(defn scene-total-steps-from-form [form]
  (* (max 1 (scene-repeat-from-form form))
     (scene-steps-from-form form)))

(defn scene-next-from-form [form]
  (scene-option-value form :next))

(defn scene-chain-info [scenes start]
  (loop [current start
         visited #{}
         chain []
         steps 0]
    (if (contains? visited current)
      {:steps (max 1 steps)
       :chain chain
       :closed? true
       :looping? false}
      (if-let [form (get scenes current)]
        (let [repeat-count (scene-repeat-from-form form)
              scene-steps (scene-steps-from-form form)
              chain (conj chain current)]
          (if (zero? repeat-count)
            {:steps scene-steps
             :chain chain
             :closed? true
             :looping? true}
            (let [steps (+ steps (* repeat-count scene-steps))]
              (if-let [next-scene (scene-next-from-form form)]
                (if (contains? scenes next-scene)
                  (recur next-scene (conj visited current) chain steps)
                  (throw (ex-info (str "scene '" current "' :next references unknown scene '" next-scene "'")
                                  {:scene current :next next-scene})))
                {:steps (max 1 steps)
                 :chain chain
                 :closed? false
                 :looping? false}))))
        nil))))

(defn played-scene [forms]
  (some (fn [form]
          (when (and (seq? form)
                     (contains? #{'play-scene 'play-block 'cue} (form-head form)))
            (second form)))
        forms))

(defn inferred-loop-steps [source]
  (let [forms (read-source-forms source)
        scenes (into {} (keep #(when-let [name (scene-name %)]
                                 [name %])
                              forms))]
    (if-let [scene (played-scene forms)]
      (if (contains? scenes scene)
        (or (:steps (scene-chain-info scenes scene))
            16)
        (throw (ex-info (str "unknown scene '" scene "'") {:scene scene})))
      (let [track-steps (map track-loop-steps (filter top-level-track? forms))]
        (if (seq track-steps)
          (lcm-int track-steps)
          1)))))

(defn emit-form [expr]
  (cond
    (seq? expr) (str "(" (clojure.string/join " " (map emit-form expr)) ")")
    (vector? expr) (str "[" (clojure.string/join " " (map emit-form expr)) "]")
    (keyword? expr) (str ":" (name expr))
    (symbol? expr) (name expr)
    (string? expr) (pr-str expr)
    (ratio? expr) (str (double expr))
    :else (str expr)))

(defn split-scene-options-and-body [scene-form]
  (loop [remaining (drop 2 scene-form)
         options []]
    (cond
      (empty? remaining) [options []]
      (keyword? (first remaining)) (recur (nnext remaining)
                                          (conj options [(first remaining) (second remaining)]))
      :else [options remaining])))

(def loop-preview-cycles 6)
(def loop-boundary-fade-ms 0.5)

(def repeat-option-keys
  #{:repeat :repeats :times :loop})

(def next-option-keys
  #{:next})

(defn scene-form-with-repeat [scene-form repeat-count]
  (let [[options body] (split-scene-options-and-body scene-form)
        options-without-repeat (remove #(contains? repeat-option-keys (first %)) options)
        flattened-options (mapcat identity (conj (vec options-without-repeat)
                                                 [:repeat repeat-count]))]
    (apply list (concat [(first scene-form) (second scene-form)]
                        flattened-options
                        body))))

(defn scene-form-with-next [scene-form next-scene]
  (let [[options body] (split-scene-options-and-body scene-form)
        options-without-next (remove #(contains? next-option-keys (first %)) options)
        flattened-options (mapcat identity (conj (vec options-without-next)
                                                 [:next next-scene]))]
    (apply list (concat [(first scene-form) (second scene-form)]
                        flattened-options
                        body))))

(defn looped-scene-form [scene-form scene]
  (if (= (scene-name scene-form) scene)
    (scene-form-with-repeat scene-form
                            (* loop-preview-cycles
                               (scene-repeat-from-form scene-form)))
    scene-form))

(defn loop-render-source [compiled-source]
  (let [forms (read-source-forms compiled-source)
        scene (played-scene forms)
        scenes (into {} (keep #(when-let [name (scene-name %)]
                                 [name %])
                              forms))
        chain-info (when scene (scene-chain-info scenes scene))]
    (if (and scene chain-info)
      (let [chain (:chain chain-info)
            last-scene (last chain)
            needs-link? (and (not (:closed? chain-info)) last-scene)]
        (->> forms
             (map #(if (and needs-link? (= (scene-name %) last-scene))
                     (scene-form-with-next % scene)
                     %))
             (map emit-form)
             (clojure.string/join "\n\n")))
      compiled-source)))

(defn read-all-bytes [input-stream]
  (let [out (ByteArrayOutputStream.)]
    (io/copy input-stream out)
    (.toByteArray out)))

(defn int16-le [bytes offset]
  (let [lo (bit-and (aget bytes offset) 0xff)
        hi (bit-and (aget bytes (inc offset)) 0xff)
        value (bit-or lo (bit-shift-left hi 8))]
    (if (>= value 32768)
      (- value 65536)
      value)))

(defn write-int16-le! [bytes offset value]
  (let [bounded (int (max -32768 (min 32767 (Math/round (double value)))))
        unsigned (if (neg? bounded) (+ 65536 bounded) bounded)]
    (aset-byte bytes offset (unchecked-byte (bit-and unsigned 0xff)))
    (aset-byte bytes (inc offset) (unchecked-byte (bit-and (bit-shift-right unsigned 8) 0xff)))))

(defn fade-gain [idx fade-frames]
  (if (pos? fade-frames)
    (/ idx (double fade-frames))
    1.0))

(defn apply-loop-boundary-fade [bytes ^AudioFormat format frame-count]
  (if (and (= 16 (.getSampleSizeInBits format))
           (not (.isBigEndian format))
           (pos? frame-count))
    (let [frame-size (.getFrameSize format)
          channels (.getChannels format)
          fade-frames (min frame-count
                           (max 1 (long (Math/round (* (.getFrameRate format)
                                                       (/ loop-boundary-fade-ms 1000.0))))))
          result (byte-array bytes)]
      (doseq [channel (range channels)]
        (dotimes [frame fade-frames]
          (let [in-offset (+ (* frame frame-size) (* channel 2))
                gain (fade-gain frame fade-frames)]
            (write-int16-le! result in-offset (* (int16-le result in-offset) gain)))
          (let [out-frame (- frame-count 1 frame)
                out-offset (+ (* out-frame frame-size) (* channel 2))
                gain (fade-gain frame fade-frames)]
            (write-int16-le! result out-offset (* (int16-le result out-offset) gain)))))
      result)
    bytes))

(defn extract-loop-cycle-wav! [^File rendered ^File wav cycle-seconds]
  (with-open [stream (AudioSystem/getAudioInputStream rendered)]
    (let [format (.getFormat stream)
          frame-size (.getFrameSize format)
          frame-length (.getFrameLength stream)
          bytes (read-all-bytes stream)
          available-frames (quot (count bytes) frame-size)
          total-frames (if (pos? frame-length)
                         (min frame-length available-frames)
                         available-frames)
          cycle-frames (max 1 (min total-frames
                                   (long (Math/round (* cycle-seconds (.getFrameRate format))))))
          start-frame (max 0 (- total-frames cycle-frames))
          start-byte (* start-frame frame-size)
          byte-count (* cycle-frames frame-size)
          selected (byte-array byte-count)]
      (System/arraycopy bytes start-byte selected 0 byte-count)
      (let [smoothed (apply-loop-boundary-fade selected format cycle-frames)]
        (with-open [out-stream (AudioInputStream.
                                 (ByteArrayInputStream. smoothed)
                                 format
                                 cycle-frames)]
          (AudioSystem/write out-stream AudioFileFormat$Type/WAVE wav))))))

(defn render-audio!
  ([frame editor status wav-name source loop? play?]
   (render-audio! frame editor status wav-name source loop? play? nil))
  ([^JFrame frame ^JTextComponent editor ^JLabel status wav-name source loop? play? render-seconds]
  (future
    (try
      (swap! state assoc :rendering true)
      (SwingUtilities/invokeLater #(set-status! status "rendering..."))
      (validate-delimiters! source)
      (let [source-file (current-file-or-session!)
            preview (preview-source source)
            _ (require-playback-form! preview)
            _ (write-file! source-file source)
            renders (File. "renders")
            _ (.mkdirs renders)
            compiled (compile-glitchlisp-source preview)
            compiled-file (File. renders "swing-compiled.gl")
            wav (wav-file-for-name wav-name)
            parent (.getParentFile wav)
            _ (when parent (.mkdirs parent))
            renderer (ensure-renderer! status)
            loop-steps (when (and loop? (nil? render-seconds))
                         (inferred-loop-steps compiled))
            effective-render-seconds (or render-seconds
                                         (when loop-steps
                                           (seconds-for-steps compiled loop-steps)))
            loop-warmup? (and loop? effective-render-seconds)
            render-source (if loop-warmup?
                            (loop-render-source compiled)
                            compiled)
            _ (write-file! compiled-file render-source)
            rendered-wav (if loop-warmup?
                           (File. renders "swing-loop-warmup.wav")
                           wav)
            render-seconds-total (if loop-warmup?
                                   (* effective-render-seconds loop-preview-cycles)
                                   effective-render-seconds)
            render-args (cond-> [renderer "render" (.getPath compiled-file) (.getPath rendered-wav)]
                          render-seconds-total
                          (conj "--seconds" (format "%.6f" render-seconds-total)))
            output (run-command! render-args)
            _ (when loop-warmup?
                (extract-loop-cycle-wav! rendered-wav wav effective-render-seconds))]
        (when play?
          (play-wav! wav loop?))
        (SwingUtilities/invokeLater
          #(set-status! status (str (if play? "playing " "saved audio ")
                                    (.getPath wav)
                                    " | "
                                    (clojure.string/trim output)))))
      (catch Exception ex
        (SwingUtilities/invokeLater
          #(do
             (report-source-error! editor status ex)
             (JOptionPane/showMessageDialog frame (editor/clean-error-message ex) "Render/play failed" JOptionPane/ERROR_MESSAGE))))
      (finally
        (swap! state assoc :rendering false))))))

(defn render-and-play!
  ([frame editor status wav-name source loop?]
   (render-and-play! frame editor status wav-name source loop? nil))
  ([frame editor status wav-name source loop? render-seconds]
   (render-audio! frame editor status wav-name source loop? true render-seconds)))
