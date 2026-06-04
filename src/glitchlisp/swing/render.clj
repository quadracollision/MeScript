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
(def validate-delimiters! editor/validate-delimiters!)

(defn choose-file [parent mode]
  (let [chooser (JFileChooser. ".")]
    (when (= JFileChooser/APPROVE_OPTION (.showDialog chooser parent mode))
      (.getSelectedFile chooser))))

(defn read-file [^File file]
  (slurp (.getPath file)))

(defn write-file! [^File file text]
  (spit (.getPath file) text))

(defn compile-glitchlisp-source [source]
  (if (.exists (File. "src/compiler.clj"))
    (load-file "src/compiler.clj")
    (when-let [compiler-source (resource-slurp "compiler.clj")]
      (load-string compiler-source)))
  (let [compiler (ns-resolve 'glitchlisp-compiler 'compile-source)]
    (if compiler
      (compiler source)
      source)))

(defn current-file-or-session! []
  (or (:file @state)
      (let [file (File. "mescript-swing-session.gl")]
        (swap! state assoc :file file)
        file)))

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
  #{"null-params" "empty-gate-silent" "gui-live" "live-audio-info" "check-live-source"})

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

(defn strip-playback-commands [source]
  (->> (clojure.string/split-lines source)
       (remove #(re-matches #"\s*\((start!|play-scene|play-block|cue)(\s+.*)?\)\s*" %))
       (clojure.string/join "\n")
       clojure.string/trim))

(defn source-with-cue [source scene]
  (str (strip-playback-commands source) "\n\n(play-scene :" scene ")\n"))

(defn has-play-command? [source]
  (boolean (re-find #"\((start!|play-scene|play-block|cue)\b" source)))

(defn has-track-form? [source]
  (boolean (re-find #"\(d\s+:" source)))

(defn first-scene-name [source]
  (some-> (re-find #"\((?:scene|block)\s+:([^\s)]+)" source)
          second))

(defn preview-source [source]
  source)

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
  (if-let [[_ bpm] (re-find #"\(bpm\s+([0-9]+(?:\.[0-9]+)?)\)" source)]
    (Double/parseDouble bpm)
    124.0))

(defn seconds-for-steps [source steps]
  (/ steps (/ (* (bpm-from-source source) 4.0) 60.0)))

(defn read-source-forms [source]
  (binding [*read-eval* false]
    (with-open [reader (PushbackReader. (StringReader. source))]
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

(defn positive-int-value [value fallback]
  (if (number? value)
    (max 1 (int value))
    fallback))

(defn non-negative-int-value [value fallback]
  (if (number? value)
    (max 0 (int value))
    fallback))

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
  (and (number? value) (not (zero? value))))

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

(defn gate-step-bools [expr]
  (if (vector? expr)
    (if (empty? expr)
      [false]
      (let [children (mapv gate-step-bools expr)
            cell-width (lcm-int (map #(max 1 (count %)) children))]
        (vec (mapcat #(expand-gate-cell % cell-width) children))))
    (if (and (seq? expr) (= 'gate-hold (form-head expr)))
      [true]
      [(truthy-gate? expr)])))

(defn euclid-bools [pulses steps rotation]
  (let [steps (max 1 (int steps))
        pulses (min (max 0 (int pulses)) steps)
        rotation (mod (int rotation) steps)]
    (mapv (fn [idx]
            (let [rotated (mod (+ idx steps (- rotation)) steps)]
              (< (mod (* rotated pulses) steps) pulses)))
          (range steps))))

(defn gate-summary-from-steps [steps]
  (let [expanded (if (seq steps) (mapv gate-step-bools steps) [[false]])
        slots (reduce + (map count expanded))
        hits (reduce + (map #(count (filter true? %)) expanded))]
    {:length (max 1 (count expanded))
     :hits hits
     :slots (max 1 slots)}))

(defn gate-pattern-summary [expr]
  (cond
    (and (seq? expr) (= 'euclid (form-head expr)))
    (let [[_ pulses steps] expr]
      (gate-summary-from-steps (mapv #(if % 1 0) (euclid-bools pulses steps 0))))

    (and (seq? expr) (= 'euclid-rot (form-head expr)))
    (let [[_ pulses steps rotation] expr]
      (gate-summary-from-steps (mapv #(if % 1 0) (euclid-bools pulses steps rotation))))

    (and (seq? expr) (= 'rev (form-head expr)))
    (gate-pattern-summary (second expr))

    (and (seq? expr) (= 'p (form-head expr)) (vector? (second expr)))
    (gate-summary-from-steps (second expr))

    (vector? expr)
    (gate-summary-from-steps expr)

    :else
    (gate-summary-from-steps [expr])))

(defn note-pattern-summary [expr]
  (cond
    (and (seq? expr) (= 'rev (form-head expr)))
    (note-pattern-summary (second expr))

    (and (seq? expr) (= 'p (form-head expr)) (vector? (second expr)))
    {:mode :step :length (max 1 (count (second expr)))}

    (and (seq? expr) (= 's (form-head expr)) (vector? (second expr)))
    {:mode :hit :length (max 1 (count (second expr)))}

    (and (seq? expr)
         (contains? #{'gs 'gate-seq 'gate_seq} (form-head expr))
         (vector? (second expr)))
    {:mode :tick :length (max 1 (count (second expr)))}

    (vector? expr)
    {:mode :step :length (max 1 (count expr))}

    :else
    {:mode :step :length 1}))

(defn track-loop-steps [track-form]
  (let [items (drop 2 track-form)
        gate (or (pair-value items :gate) 1)
        note (or (pair-value items :note) 'c3)
        every (positive-int-value (pair-value items :every) 1)
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
  (and (seq? form) (= 'd (form-head form))))

(defn scene-form? [form]
  (and (seq? form) (contains? #{'scene 'block} (form-head form))))

(defn scene-name [form]
  (when (scene-form? form)
    (second form)))

(defn scene-option-value [form key]
  (pair-value (drop 2 form) key))

(defn scene-body-forms [scene-form]
  (loop [remaining (drop 2 scene-form)]
    (cond
      (empty? remaining) []
      (keyword? (first remaining)) (recur (nnext remaining))
      :else remaining)))

(defn track-id [track-form]
  (when (top-level-track? track-form)
    (second track-form)))

(defn scene-inferred-steps [form]
  (let [body (scene-body-forms form)
        tracks (filter top-level-track? body)]
    (if-let [target (or (scene-option-value form :steps-of)
                        (scene-option-value form :length-of))]
      (if-let [track (some #(when (= target (track-id %)) %) tracks)]
        (track-loop-steps track)
        16)
      (if (seq tracks)
        (lcm-int (map track-loop-steps tracks))
        16))))

(defn scene-steps-from-form [form]
  (if-let [explicit (or (scene-option-value form :steps)
                        (scene-option-value form :length)
                        (scene-option-value form :bars))]
    (positive-int-value explicit 16)
    (scene-inferred-steps form)))

(defn scene-repeat-from-form [form]
  (non-negative-int-value (or (scene-option-value form :repeat)
                              (scene-option-value form :repeats)
                              (scene-option-value form :times))
                          0))

(defn scene-total-steps-from-form [form]
  (* (max 1 (scene-repeat-from-form form))
     (scene-steps-from-form form)))

(defn played-scene [forms]
  (some (fn [form]
          (when (and (seq? form)
                     (contains? #{'play-scene 'play-block 'cue} (form-head form)))
            (second form)))
        forms))

(defn inferred-loop-steps [source]
  (let [forms (read-source-forms source)
        scenes (into {} (keep #(when-let [name (scene-name %)]
                                 [name (scene-total-steps-from-form %)])
                              forms))]
    (if-let [scene (played-scene forms)]
      (get scenes scene 16)
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
  #{:repeat :repeats :times})

(defn scene-form-with-repeat [scene-form repeat-count]
  (let [[options body] (split-scene-options-and-body scene-form)
        options-without-repeat (remove #(contains? repeat-option-keys (first %)) options)
        flattened-options (mapcat identity (conj (vec options-without-repeat)
                                                 [:repeat repeat-count]))]
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
        scene (played-scene forms)]
    (if scene
      (->> forms
           (map #(if (scene-name %)
                   (looped-scene-form % scene)
                   %))
           (map emit-form)
           (clojure.string/join "\n\n"))
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
      (require-playback-form! source)
      (let [source-file (current-file-or-session!)
            preview (preview-source source)
            _ (write-file! source-file preview)
            renders (File. "renders")
            _ (.mkdirs renders)
            compiled (compile-glitchlisp-source preview)
            compiled-file (File. renders "swing-compiled.gl")
            _ (write-file! compiled-file compiled)
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
             (focus-source-error! editor status ex)
             (JOptionPane/showMessageDialog frame (editor/clean-error-message ex) "Render/play failed" JOptionPane/ERROR_MESSAGE)))
        (when-not (:offset (ex-data ex))
          (SwingUtilities/invokeLater #(set-status! status "render/play failed"))))
      (finally
        (swap! state assoc :rendering false))))))

(defn render-and-play!
  ([frame editor status wav-name source loop?]
   (render-and-play! frame editor status wav-name source loop? nil))
  ([frame editor status wav-name source loop? render-seconds]
   (render-audio! frame editor status wav-name source loop? true render-seconds)))
