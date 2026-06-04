(ns glitchlisp-compiler
  (:require [clojure.string :as str])
  (:import [java.io PushbackReader]))

(defn gate-hold-token? [token]
  (boolean (re-matches #"1_(?:[0-9]+)?" token)))

(defn gate-hold-replacement [token]
  (let [amount-text (subs token 2)
        amount (if (clojure.string/blank? amount-text) "1" amount-text)]
    (str "(gate-hold " amount ")")))

(defn rewrite-gate-hold-tokens [source]
  (let [token-end? #(or (Character/isWhitespace ^char %)
                        (contains? #{\( \) \[ \] \;} %))]
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

(defn read-forms [source]
  (with-open [reader (PushbackReader. (java.io.StringReader. source))]
    (loop [forms []]
      (let [form (read reader false ::eof)]
        (if (= form ::eof)
          forms
          (recur (conj forms form)))))))

(declare expand-expr)

(defn tracks-form? [expr]
  (and (seq? expr) (= 'tracks (first expr))))

(defn track-form? [expr]
  (and (seq? expr) (= 'd (first expr))))

(defn scene-form? [expr]
  (and (seq? expr) (contains? #{'scene 'block} (first expr))))

(defn def-form? [expr]
  (and (seq? expr) (= 'def (first expr))))

(defn def-binding [form]
  (let [[_ name & values] form]
    (when (empty? values)
      (throw (ex-info "def requires a name and at least one value" {:form form})))
    (when-not (symbol? name)
      (throw (ex-info "def name must be a symbol" {:form form})))
    [name (if (= 1 (count values))
            (first values)
            (apply list 'tracks values))]))

(def ^:dynamic *captured-defs* nil)
(def ^:dynamic *scene-context* nil)

(defn by-scene-value [args]
  (when (odd? (count args))
    (throw (ex-info "by-scene expects scene/value pairs and optional :else value" {:args args})))
  (let [pairs (partition 2 args)
        scene-key (when *scene-context* (keyword *scene-context*))]
    (or (some (fn [[key value]]
                (when (= key scene-key) value))
              pairs)
        (some (fn [[key value]]
                (when (= key :else) value))
              pairs)
        (throw (ex-info (str "by-scene has no value for scene " scene-key " and no :else")
                        {:scene scene-key
                         :args args})))))

(defn repeat-pattern [n values]
  (vec (apply concat (repeat n values))))

(def note-offsets
  {"c" 0 "cs" 1 "db" 1
   "d" 2 "ds" 3 "eb" 3
   "e" 4
   "f" 5 "fs" 6 "gb" 6
   "g" 7 "gs" 8 "ab" 8
   "a" 9 "as" 10 "bb" 10
   "b" 11})

(def midi-names
  ["c" "db" "d" "eb" "e" "f" "gb" "g" "ab" "a" "bb" "b"])

(def scales
  {:major [0 2 4 5 7 9 11]
   :minor [0 2 3 5 7 8 10]
   :natural-minor [0 2 3 5 7 8 10]
   :harmonic-minor [0 2 3 5 7 8 11]
   :melodic-minor [0 2 3 5 7 9 11]
   :pentatonic [0 2 4 7 9]
   :minor-pentatonic [0 3 5 7 10]
   :blues [0 3 5 6 7 10]
   :dorian [0 2 3 5 7 9 10]
   :phrygian [0 1 3 5 7 8 10]
   :lydian [0 2 4 6 7 9 11]
   :mixolydian [0 2 4 5 7 9 10]
   :locrian [0 1 3 5 6 8 10]
   :chromatic [0 1 2 3 4 5 6 7 8 9 10 11]})

(def chords
  {:major [0 4 7]
   :minor [0 3 7]
   :dim [0 3 6]
   :diminished [0 3 6]
   :aug [0 4 8]
   :augmented [0 4 8]
   :sus2 [0 2 7]
   :sus4 [0 5 7]
   :power [0 7]
   :7 [0 4 7 10]
   :dom7 [0 4 7 10]
   :m7 [0 3 7 10]
   :minor7 [0 3 7 10]
   :maj7 [0 4 7 11]
   :major7 [0 4 7 11]})

(defn note-symbol? [value]
  (and (symbol? value)
       (boolean (re-matches #"[a-gA-G](?:s|b)?-?\d+" (name value)))))

(defn note->midi [value]
  (when-not (note-symbol? value)
    (throw (ex-info "expected note symbol" {:value value})))
  (let [[_ root octave-text] (re-matches #"([a-gA-G](?:s|b)?)(-?\d+)" (name value))
        offset (get note-offsets (str/lower-case root))]
    (when-not offset
      (throw (ex-info "unknown note" {:value value})))
    (+ 57 (* (- (Integer/parseInt octave-text) 4) 12) offset)))

(defn midi->note [midi]
  (let [relative (- midi 57)
        octave (+ 4 (Math/floorDiv relative 12))
        pitch (mod relative 12)]
    (symbol (str (midi-names pitch) octave))))

(defn numeric-value [value]
  (if (number? value)
    value
    (throw (ex-info "expected number" {:value value}))))

(defn bool-number [value]
  (if value 1 0))

(declare boolean-form)

(defn truthy-number? [value]
  (not (zero? (numeric-value value))))

(defn boolean-not-value [value]
  (if (vector? value)
    (mapv boolean-not-value value)
    (bool-number (not (truthy-number? value)))))

(defn boolean-combine-values [op values]
  (let [vectors (filter vector? values)]
    (if (seq vectors)
      (let [n (apply min (map count vectors))]
        (mapv (fn [idx]
                (let [cell-values (map (fn [value]
                                         (if (vector? value)
                                           (value idx)
                                           value))
                                       values)]
                  (boolean-form op cell-values)))
              (range n)))
      (case op
        and (bool-number (every? truthy-number? values))
        or (bool-number (some truthy-number? values))))))

(defn positive-count [value form-name]
  (let [n (numeric-value value)]
    (when (neg? n)
      (throw (ex-info (str form-name " count must be non-negative") {:value value})))
    (int n)))

(defn vector-value [value form-name]
  (if (vector? value)
    value
    (throw (ex-info (str form-name " requires a vector") {:value value}))))

(defn transpose-value [value semitones]
  (if (note-symbol? value)
    (midi->note (+ (note->midi value) (int (numeric-value semitones))))
    (+ (numeric-value value) (numeric-value semitones))))

(defn arithmetic-form [op values]
  (when (empty? values)
    (throw (ex-info "arithmetic form requires at least one value" {:op op})))
  (let [first-value (first values)]
    (if (and (= '+ op) (note-symbol? first-value))
      (reduce transpose-value first-value (rest values))
      (case op
        + (reduce + (map numeric-value values))
        - (if (= 1 (count values))
            (- (numeric-value first-value))
            (reduce - (map numeric-value values)))
        * (reduce * (map numeric-value values))
        / (if (= 1 (count values))
            (/ 1 (numeric-value first-value))
            (reduce / (map numeric-value values)))))))

(defn boolean-form [op values]
  (case op
    and (boolean-combine-values 'and values)
    or (boolean-combine-values 'or values)
    not (do
          (when-not (= 1 (count values))
            (throw (ex-info "not expects exactly one value" {:args values})))
          (boolean-not-value (first values)))))

(defn generated-range [args]
  (let [[start end step] (case (count args)
                           1 [0 (first args) 1]
                           2 [(first args) (second args) 1]
                           3 args
                           (throw (ex-info "range expects end, start/end, or start/end/step" {:args args})))
        step (numeric-value step)]
    (when (zero? step)
      (throw (ex-info "range step cannot be zero" {:args args})))
    (if (or (note-symbol? start) (note-symbol? end))
      (let [start-midi (note->midi start)
            end-midi (note->midi end)]
        (mapv midi->note (range start-midi end-midi (int step))))
      (vec (range (numeric-value start) (numeric-value end) step)))))

(defn generated-repeat [args]
  (let [[n value & extra] args]
    (when extra
      (throw (ex-info "repeat expects count and one value" {:args args})))
    (let [n (positive-count n "repeat")]
      (if (vector? value)
        (vec (apply concat (repeat n value)))
        (vec (repeat n value))))))

(defn generated-take [args]
  (let [[n value & extra] args]
    (when extra
      (throw (ex-info "take expects count and one vector" {:args args})))
    (let [n (positive-count n "take")
          values (vector-value value "take")]
      (vec (take n values)))))

(defn generated-reverse [args]
  (let [[value & extra] args]
    (when extra
      (throw (ex-info "reverse expects one vector" {:args args})))
    (vec (reverse (vector-value value "reverse")))))

(defn generated-rotate [args]
  (let [[n value & extra] args]
    (when extra
      (throw (ex-info "rotate expects amount and one vector" {:args args})))
    (let [values (vector-value value "rotate")
          size (count values)]
      (if (zero? size)
        []
        (let [amount (mod (int (numeric-value n)) size)]
          (vec (concat (drop amount values) (take amount values))))))))

(defn generated-interleave [args]
  (let [vectors (map #(vector-value % "interleave") args)]
    (when (empty? vectors)
      (throw (ex-info "interleave expects at least one vector" {:args args})))
    (vec (apply concat (apply map vector vectors)))))

(defn generated-every-n [args]
  (let [[n hit & more] args
        [rest-value extra] (case (count more)
                             0 [0 nil]
                             1 [(first more) nil]
                             [(first more) (rest more)])]
    (when (seq extra)
      (throw (ex-info "every-n expects n, hit value, and optional rest value" {:args args})))
    (let [n (max 1 (positive-count n "every-n"))]
      (mapv (fn [idx] (if (zero? (mod idx n)) hit rest-value))
            (range n)))))

(defn seeded-rand [seed]
  (let [next-seed (mod (+ (* 1664525 (long seed)) 1013904223) 4294967296)]
    [next-seed (/ next-seed 4294967296.0)]))

(defn option-value [args key default]
  (let [pairs (partition 2 args)]
    (if-let [match (some (fn [[k v]] (when (= k key) v)) pairs)]
      match
      default)))

(defn generated-choose [args]
  (let [seed (long (numeric-value (option-value args :seed 1)))
        n (positive-count (option-value args :count 8) "choose")
        values (vector-value (last args) "choose")]
    (when (empty? values)
      (throw (ex-info "choose requires a non-empty vector" {:args args})))
    (loop [seed seed
           out []]
      (if (= (count out) n)
        out
        (let [[next-seed r] (seeded-rand seed)
              idx (int (Math/floor (* r (count values))))]
          (recur next-seed (conj out (values (min idx (dec (count values)))))))))))

(defn generated-rand-range [args]
  (let [seed (long (numeric-value (option-value args :seed 1)))
        n (positive-count (option-value args :count 8) "rand-range")
        min-value (numeric-value (option-value args :min 0))
        max-value (numeric-value (option-value args :max 1))]
    (loop [seed seed
           out []]
      (if (= (count out) n)
        out
        (let [[next-seed r] (seeded-rand seed)]
          (recur next-seed (conj out (+ min-value (* r (- max-value min-value))))))))))

(defn generated-scale [args]
  (let [[root scale-name scale-count & extra] args]
    (when extra
      (throw (ex-info "scale expects root, scale name, and count" {:args args})))
    (let [intervals (or (get scales scale-name)
                        (throw (ex-info "unknown scale" {:scale scale-name})))
          root-midi (note->midi root)
          n (positive-count scale-count "scale")
          width (count intervals)]
      (mapv (fn [idx]
              (midi->note (+ root-midi
                             (intervals (mod idx width))
                             (* 12 (quot idx width)))))
            (range n)))))

(defn generated-chord [args]
  (let [[root chord-name & extra] args]
    (when extra
      (throw (ex-info "chord expects root and chord name" {:args args})))
    (let [intervals (or (get chords chord-name)
                        (throw (ex-info "unknown chord" {:chord chord-name})))
          root-midi (note->midi root)]
      (mapv #(midi->note (+ root-midi %)) intervals))))

(defn expand-list-items [env items]
  (loop [remaining items
         local-env env
         output []]
    (if (seq remaining)
      (let [item (first remaining)]
        (if (def-form? item)
          (let [[name value] (def-binding item)]
            (let [expanded (expand-expr local-env value)
                  emitted (if (tracks-form? expanded) (vec (rest expanded)) [expanded])]
              (when *captured-defs*
                (swap! *captured-defs* conj [name expanded]))
              (recur (rest remaining)
                     (assoc local-env name expanded)
                     (into output emitted))))
          (let [expanded (expand-expr local-env item)
                emitted (if (tracks-form? expanded) (vec (rest expanded)) [expanded])]
            (recur (rest remaining)
                   local-env
                   (into output emitted)))))
      output)))

(defn expand-p-form [env args]
  (if (and (= :repeat (first args)) (>= (count args) 3))
    (let [n-form (second args)
          pattern-form (nth args 2)
          n (if (number? n-form)
              (int n-form)
              (throw (ex-info "p :repeat requires a numeric repeat count" {:form n-form})))
          expanded-pattern (expand-expr env pattern-form)]
      (when-not (vector? expanded-pattern)
        (throw (ex-info "p :repeat requires a vector pattern" {:form pattern-form})))
      (list 'p (repeat-pattern (max 0 n) expanded-pattern)))
    (apply list 'p (expand-list-items env args))))

(defn expand-map-form [env args]
  (let [[op & sources] args
        expanded-sources (mapv #(expand-expr env %) sources)
        vectors (filter vector? expanded-sources)]
    (when (empty? expanded-sources)
      (throw (ex-info "map expects an operation and at least one value source" {:args args})))
    (when (empty? vectors)
      (throw (ex-info "map expects at least one vector source" {:args args})))
    (let [n (apply min (map count vectors))]
      (mapv (fn [idx]
              (let [values (map (fn [source]
                                  (if (vector? source)
                                    (source idx)
                                    source))
                                expanded-sources)]
                (expand-expr env (apply list op values))))
            (range n)))))

(defn expand-expr [env expr]
  (cond
    (symbol? expr)
    (if (contains? env expr)
      (expand-expr env (get env expr))
      expr)

    (vector? expr)
    (vec (expand-list-items env expr))

    (seq? expr)
    (let [head (first expr)
          args (rest expr)
          expanded-args (delay (map #(expand-expr env %) args))]
      (case head
        by-scene (if *scene-context*
                   (expand-expr env (by-scene-value args))
                   (apply list 'by-scene (expand-list-items env args)))
        + (arithmetic-form '+ @expanded-args)
        - (arithmetic-form '- @expanded-args)
        * (arithmetic-form '* @expanded-args)
        / (arithmetic-form '/ @expanded-args)
        and (boolean-form 'and @expanded-args)
        or (boolean-form 'or @expanded-args)
        not (boolean-form 'not @expanded-args)
        range (generated-range @expanded-args)
        repeat (generated-repeat @expanded-args)
        take (generated-take @expanded-args)
        reverse (if (and (= 1 (count @expanded-args))
                         (vector? (first @expanded-args)))
                  (generated-reverse @expanded-args)
                  (apply list (expand-list-items env expr)))
        rotate (generated-rotate @expanded-args)
        interleave (generated-interleave @expanded-args)
        every-n (generated-every-n @expanded-args)
        choose (generated-choose @expanded-args)
        rand-range (generated-rand-range @expanded-args)
        scale (generated-scale @expanded-args)
        chord (generated-chord @expanded-args)
        transpose (let [[value semitones & extra] @expanded-args]
                    (when extra
                      (throw (ex-info "transpose expects value and semitones" {:args @expanded-args})))
                    (transpose-value value semitones))
        map (expand-map-form env args)
        p (expand-p-form env args)
        tracks (apply list 'tracks (expand-list-items env args))
        (apply list (expand-list-items env expr))))

    :else expr))

(defn compile-forms [forms]
  (loop [remaining forms
         env {}
         output []]
    (if-let [form (first remaining)]
      (if (def-form? form)
        (let [[name value] (def-binding form)]
          (recur (rest remaining)
                 (assoc env name (expand-expr env value))
                 output))
        (let [scene-context (when (scene-form? form)
                              (second form))
              captured-defs (atom [])
              expanded (binding [*captured-defs* captured-defs]
                         (binding [*scene-context* scene-context]
                           (expand-expr env form)))
              emitted (if (tracks-form? expanded) (vec (rest expanded)) [expanded])
              output (reduce (fn [result expr]
                               (if (and (track-form? expr)
                                        (seq result)
                                        (scene-form? (peek result)))
                                 (conj (pop result) (concat (peek result) [expr]))
                                 (conj result expr)))
                             output
                             emitted)
              env (into env @captured-defs)]
          (recur (rest remaining) env output)))
      output)))

(defn emit [expr]
  (cond
    (nil? expr) "nil"
    (seq? expr) (str "(" (str/join " " (map emit expr)) ")")
    (vector? expr) (str "[" (str/join " " (map emit expr)) "]")
    (keyword? expr) (str ":" (name expr))
    (symbol? expr) (name expr)
    (string? expr) (pr-str expr)
    (ratio? expr) (str (double expr))
    :else (str expr)))

(defn compile-source [source]
  (->> (read-forms (rewrite-gate-hold-tokens source))
       compile-forms
       (map emit)
       (str/join "\n\n")))

(defn compile-file! [input output]
  (spit output (str (compile-source (slurp input)) "\n")))

(defn -main [& args]
  (let [[input output] args]
    (when-not (and input output)
      (binding [*out* *err*]
        (println "usage: clojure src/compiler.clj <input.gl> <output.gl>"))
      (System/exit 2))
    (compile-file! input output)))
