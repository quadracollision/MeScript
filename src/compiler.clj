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

(defn with-form? [expr]
  (and (seq? expr) (= 'with (first expr))))

(defn section-form? [expr]
  (and (seq? expr) (= 'section (first expr))))

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

(def scene-option-arity
  {:loop 1
   :repeat 1
   :repeats 1
   :times 1
   :steps 1
   :length 1
   :bars 1
   :bar-steps 1
   :bar-length 1
   :bar-steps-of 1
   :bar-length-of 1
   :steps-of 1
   :length-of 1
   :loop-by 2
   :next 1})

(defn split-scene-options [form-name args]
  (loop [remaining args
         options []]
    (if (and (seq remaining) (keyword? (first remaining)))
      (let [key (first remaining)
            arity (or (get scene-option-arity key)
                      (throw (ex-info (str form-name " unknown option " key) {:key key
                                                                               :args args})))
            values (take arity (rest remaining))]
        (when-not (= arity (count values))
          (throw (ex-info (str form-name " " key " requires "
                               (if (= 1 arity) "a value" (str arity " values")))
                          {:key key
                           :args args})))
        (recur (drop (inc arity) remaining)
               (into options (cons key values))))
      [options remaining])))

(defn scene-option-entries [options]
  (loop [remaining options
         entries []]
    (if (seq remaining)
      (let [key (first remaining)
            arity (get scene-option-arity key)
            values (take arity (rest remaining))]
        (recur (drop (inc arity) remaining)
               (conj entries [key values])))
      entries)))

(defn scene-option-value [options key]
  (some (fn [[option-key values]]
          (when (= option-key key)
            (first values)))
        (scene-option-entries options)))

(defn option-present? [options key]
  (boolean (some #(= key (first %)) (scene-option-entries options))))

(defn loop-option? [options]
  (option-present? options :loop))

(defn section-scene-id [scene-id idx]
  (if (zero? idx)
    scene-id
    (keyword (str (name scene-id) "__section_" (inc idx)))))

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
   :major-pentatonic [0 2 4 7 9]
   :minor-pentatonic [0 3 5 7 10]
   :blues [0 3 5 6 7 10]
   :minor-blues [0 3 5 6 7 10]
   :major-blues [0 2 3 4 7 9]
   :dorian [0 2 3 5 7 9 10]
   :phrygian [0 1 3 5 7 8 10]
   :lydian [0 2 4 6 7 9 11]
   :mixolydian [0 2 4 5 7 9 10]
   :locrian [0 1 3 5 6 8 10]
   :chromatic [0 1 2 3 4 5 6 7 8 9 10 11]
   :whole-tone [0 2 4 6 8 10]
   :diminished [0 2 3 5 6 8 9 11]
   :whole-half-diminished [0 2 3 5 6 8 9 11]
   :half-whole-diminished [0 1 3 4 6 7 9 10]
   :bebop-major [0 2 4 5 7 8 9 11]
   :bebop-dominant [0 2 4 5 7 9 10 11]})

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
    (when-not (zero? (mod n 1))
      (throw (ex-info (str form-name " count must be a whole number") {:value value})))
    (int n)))

(defn whole-number-value [value form-name label]
  (let [n (numeric-value value)]
    (when-not (zero? (mod n 1))
      (throw (ex-info (str form-name " " label " must be a whole number") {:value value})))
    (int n)))

(defn whole-number-long-value [value form-name label]
  (let [n (numeric-value value)]
    (when-not (zero? (mod n 1))
      (throw (ex-info (str form-name " " label " must be a whole number") {:value value})))
    (long n)))

(defn strictly-positive-count [value form-name]
  (let [n (positive-count value form-name)]
    (when (zero? n)
      (throw (ex-info (str form-name " count must be greater than zero") {:value value})))
    n))

(defn vector-value [value form-name]
  (if (vector? value)
    value
    (throw (ex-info (str form-name " requires a vector") {:value value}))))

(defn transpose-value [value semitones]
  (if (note-symbol? value)
    (midi->note (+ (note->midi value)
                   (whole-number-value semitones "transpose" "semitones")))
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
        (mapv midi->note (range start-midi
                                end-midi
                                (whole-number-value step "range" "note step"))))
      (vec (range (numeric-value start) (numeric-value end) step)))))

(defn generated-repeat [args]
  (when-not (= 2 (count args))
    (throw (ex-info "repeat expects count and one value" {:args args})))
  (let [[n value] args]
    (let [n (positive-count n "repeat")]
      (if (vector? value)
        (vec (apply concat (repeat n value)))
        (vec (repeat n value))))))

(defn generated-take [args]
  (when-not (= 2 (count args))
    (throw (ex-info "take expects count and one vector" {:args args})))
  (let [[n value] args
        n (positive-count n "take")
        values (vector-value value "take")]
    (vec (take n values))))

(defn generated-reverse [args]
  (let [[value & extra] args]
    (when extra
      (throw (ex-info "reverse expects one vector" {:args args})))
    (vec (reverse (vector-value value "reverse")))))

(defn generated-rotate [args]
  (when-not (= 2 (count args))
    (throw (ex-info "rotate expects amount and one vector" {:args args})))
  (let [[n value] args
        values (vector-value value "rotate")
        size (count values)]
    (if (zero? size)
      []
      (let [amount (mod (whole-number-value n "rotate" "amount") size)]
        (vec (concat (drop amount values) (take amount values)))))))

(defn generated-interleave [args]
  (let [vectors (map #(vector-value % "interleave") args)]
    (when (empty? vectors)
      (throw (ex-info "interleave expects at least one vector" {:args args})))
    (when-not (apply = (map count vectors))
      (throw (ex-info "interleave vectors must have the same length" {:args args})))
    (vec (apply concat (apply map vector vectors)))))

(defn generated-every-n [args]
  (when-not (<= 2 (count args) 3)
    (throw (ex-info "every-n expects n, hit value, and optional rest value" {:args args})))
  (let [[n hit & more] args
        [rest-value extra] (case (count more)
                             0 [0 nil]
                             1 [(first more) nil]
                             [(first more) (rest more)])]
    (when (seq extra)
      (throw (ex-info "every-n expects n, hit value, and optional rest value" {:args args})))
    (let [n (strictly-positive-count n "every-n")]
      (mapv (fn [idx] (if (zero? (mod idx n)) hit rest-value))
            (range n)))))

(defn seeded-rand [seed]
  (let [next-seed (mod (+ (* 1664525 (long seed)) 1013904223) 4294967296)]
    [next-seed (/ next-seed 4294967296.0)]))

(defn option-map [form-name allowed args]
  (loop [remaining args
         options {}]
    (cond
      (empty? remaining)
      options

      (not (keyword? (first remaining)))
      (throw (ex-info (str form-name " options must be keyword/value pairs")
                      {:args args
                       :value (first remaining)}))

      (not (contains? allowed (first remaining)))
      (throw (ex-info (str "unknown " form-name " option " (first remaining))
                      {:args args
                       :option (first remaining)}))

      (empty? (rest remaining))
      (throw (ex-info (str form-name " " (first remaining) " requires a value")
                      {:args args
                       :option (first remaining)}))

      (contains? options (first remaining))
      (throw (ex-info (str "duplicate " form-name " option " (first remaining))
                      {:args args
                       :option (first remaining)}))

      :else
      (recur (nnext remaining)
             (assoc options (first remaining) (second remaining))))))

(defn option-value [options key default]
  (if (contains? options key)
    (get options key)
    default))

(defn generated-choose [args]
  (when (empty? args)
    (throw (ex-info "choose expects options and one vector" {:args args})))
  (let [values (vector-value (last args) "choose")
        options (option-map "choose" #{:seed :count} (butlast args))
        seed (whole-number-long-value (option-value options :seed 1) "choose" "seed")
        n (positive-count (option-value options :count 8) "choose")]
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
  (let [options (option-map "rand-range" #{:seed :count :min :max} args)
        seed (whole-number-long-value (option-value options :seed 1) "rand-range" "seed")
        n (positive-count (option-value options :count 8) "rand-range")
        min-value (numeric-value (option-value options :min 0))
        max-value (numeric-value (option-value options :max 1))]
    (when (< max-value min-value)
      (throw (ex-info "rand-range :max must be greater than or equal to :min"
                      {:args args
                       :min min-value
                       :max max-value})))
    (loop [seed seed
           out []]
      (if (= (count out) n)
        out
        (let [[next-seed r] (seeded-rand seed)]
          (recur next-seed (conj out (+ min-value (* r (- max-value min-value))))))))))

(defn generated-scale [args]
  (when-not (= 3 (count args))
    (throw (ex-info "scale expects root, scale name, and count" {:args args})))
  (let [[root scale-name scale-count] args
        intervals (or (get scales scale-name)
                      (throw (ex-info "unknown scale" {:scale scale-name})))
        root-midi (note->midi root)
        n (positive-count scale-count "scale")
        width (count intervals)]
    (mapv (fn [idx]
            (midi->note (+ root-midi
                           (intervals (mod idx width))
                           (* 12 (quot idx width)))))
          (range n))))

(defn generated-chord [args]
  (when-not (= 2 (count args))
    (throw (ex-info "chord expects root and chord name or interval vector" {:args args})))
  (let [[root chord-form] args
        intervals (cond
                    (keyword? chord-form)
                    (or (get chords chord-form)
                        (throw (ex-info "unknown chord" {:chord chord-form})))

                    (vector? chord-form)
                    (mapv #(numeric-value %) chord-form)

                    :else
                    (throw (ex-info "chord expects a chord name or interval vector"
                                    {:chord chord-form})))
        root-midi (note->midi root)]
    (mapv #(midi->note (+ root-midi %)) intervals)))

(defn generated-arpeggio [args]
  (generated-chord args))

(defn generated-shape [args]
  (when-not (= 2 (count args))
    (throw (ex-info "shape expects a vector and a vector of 1-based positions" {:args args})))
  (let [[values positions] args
        values (vector-value values "shape")
        positions (vector-value positions "shape")]
    (mapv (fn [position]
            (let [n (positive-count position "shape position")]
              (when (zero? n)
                (throw (ex-info "shape positions are 1-based" {:position position})))
              (or (get values (dec n))
                  (throw (ex-info "shape position is outside the source vector"
                                  {:position position
                                   :size (count values)})))))
          positions)))

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

(defn validate-track-param-pairs! [form-name values]
  (when (odd? (count values))
    (throw (ex-info (str form-name " expects keyword/value override pairs") {:args values})))
  (doseq [[key _] (partition 2 values)]
    (when-not (keyword? key)
      (throw (ex-info (str form-name " override keys must be keywords") {:key key
                                                                          :args values})))))

(defn track-param-map [form-name params]
  (validate-track-param-pairs! form-name params)
  (loop [remaining params
         order []
         values {}]
    (if (seq remaining)
      (let [[key value & more] remaining
            canonical (track-param-canonical-key key)]
        (when (contains? values canonical)
          (throw (ex-info (str form-name " has duplicate track parameter " key) {:key key})))
        (recur more (conj order canonical) (assoc values canonical [key value])))
      [order values])))

(defn with-target-track [env target-form]
  (cond
    (and (symbol? target-form) (contains? env target-form))
    (with-target-track env (get env target-form))

    (track-form? target-form)
    target-form

    :else
    (let [expanded (expand-expr env target-form)]
      (if (track-form? expanded)
        expanded
        (throw (ex-info "with requires a track form or track def" {:target target-form
                                                                   :expanded expanded}))))))

(defn expand-with-form [env args]
  (let [target-form (first args)
        override-forms (rest args)]
    (when-not target-form
      (throw (ex-info "with requires a track and keyword/value overrides" {:args args})))
    (let [track (with-target-track env target-form)]
      (let [[_ track-id & base-params] track]
        (when-not (keyword? track-id)
          (throw (ex-info "with target track id must be a keyword" {:track track})))
        (let [overrides (expand-list-items env override-forms)
              [base-order base-values] (track-param-map "with target" base-params)
              [override-order override-values] (track-param-map "with" overrides)
              final-order (into base-order
                                (remove #(contains? base-values %) override-order))
              final-order (vec (concat (remove #(contains? override-values %) final-order)
                                       override-order))
              final-values (merge base-values override-values)]
          (expand-expr env
                       (apply list
                              'd
                              track-id
                              (mapcat #(get final-values %) final-order))))))))

(defn expand-sectioned-scene-form [env head args]
  (let [scene-id (first args)]
    (when-not (keyword? scene-id)
      (throw (ex-info "scene requires a keyword name" {:args args})))
    (let [[outer-options body] (split-scene-options "scene" (rest args))
          sections (filter section-form? body)]
      (if (empty? sections)
        (apply list head (expand-list-items env args))
        (do
          (when-not (= (count sections) (count body))
            (throw (ex-info "sectioned scene cannot mix section forms with direct tracks"
                            {:scene scene-id
                             :body body})))
          (let [outer-next (scene-option-value outer-options :next)
                unsupported-outer (remove #(= :next %) (map first (scene-option-entries outer-options)))]
            (when (seq unsupported-outer)
              (throw (ex-info "sectioned scene only supports outer :next; put repeat/loop/steps on sections"
                              {:scene scene-id
                               :options unsupported-outer})))
            (apply list
                   'tracks
                   (map-indexed
                     (fn [idx section]
                       (let [[_ & section-args] section
                             [section-options section-body] (split-scene-options "section" section-args)
                             section-id (section-scene-id scene-id idx)
                             next-id (when (< idx (dec (count sections)))
                                       (section-scene-id scene-id (inc idx)))
                             last-section? (= idx (dec (count sections)))
                             explicit-next? (option-present? section-options :next)]
                         (when (and next-id explicit-next?)
                           (throw (ex-info "non-final section cannot set :next; sections chain automatically"
                                           {:scene scene-id
                                            :section idx})))
                         (when (and next-id (loop-option? section-options))
                           (throw (ex-info "non-final section cannot loop forever"
                                           {:scene scene-id
                                            :section idx})))
                         (let [section-options (cond
                                                 next-id
                                                 (concat section-options [:next next-id])

                                                 (and last-section? outer-next (not explicit-next?))
                                                 (concat section-options [:next outer-next])

                                                 :else
                                                 section-options)]
                           (apply list
                                  head
                                  section-id
                                  (concat (expand-list-items env section-options)
                                          (expand-list-items env section-body))))))
                     sections))))))))

(defn expand-p-form [env args]
  (if (= :repeat (first args))
    (do
      (when-not (= 3 (count args))
        (throw (ex-info "p :repeat expects count and one vector pattern" {:args args})))
      (let [n-form (second args)
            pattern-form (nth args 2)
            n (if (number? n-form)
                (positive-count n-form "p :repeat")
                (throw (ex-info "p :repeat requires a numeric repeat count" {:form n-form})))
            expanded-pattern (expand-expr env pattern-form)]
        (when-not (vector? expanded-pattern)
          (throw (ex-info "p :repeat requires a vector pattern" {:form pattern-form})))
        (list 'p (repeat-pattern n expanded-pattern))))
    (let [pattern-form (first args)
          expanded-items (expand-list-items env args)]
      (when (empty? args)
        (throw (ex-info "p requires a pattern" {:args args})))
      (when (> (count args) 1)
        (if (some #(= 'then %) (rest args))
          (throw (ex-info "p wraps exactly one pattern; use (p (then A B)) instead of (p A then B)"
                          {:args args}))
          (throw (ex-info "p expects one pattern" {:args args}))))
      (if (and (= 1 (count args))
               (seq? pattern-form)
               (contains? #{'chord 'shape} (first pattern-form)))
        (list 'p [(first expanded-items)])
        (apply list 'p expanded-items)))))

(defn positive-runtime-count [value form-name]
  (when-not (number? value)
    (throw (ex-info "expected numeric pattern value" {:value value})))
  (when (or (neg? value) (not (zero? (mod value 1))))
    (throw (ex-info (str form-name " must be a non-negative integer") {:value value})))
  (when (zero? value)
    (throw (ex-info (str form-name " must be greater than zero") {:value value})))
  (int value))

(defn expand-times-form [env args]
  (let [count-form (first args)
        pattern-form (second args)]
    (when-not count-form
      (throw (ex-info "times requires a count" {:args args})))
    (when-not pattern-form
      (throw (ex-info "times requires a pattern" {:args args})))
    (when-not (= 2 (count args))
      (throw (ex-info "times expects count and one pattern" {:args args})))
    (let [expanded-count (expand-expr env count-form)
          _ (positive-runtime-count expanded-count "times")
          expanded-pattern (expand-expr env pattern-form)]
      (list 'times expanded-count expanded-pattern))))

(defn expand-then-form [env args]
  (when (< (count args) 2)
    (throw (ex-info "then expects at least two patterns" {:args args})))
  (apply list 'then (map #(expand-expr env %) args)))

(defn expand-map-form [env args]
  (let [[op & sources] args
        expanded-sources (mapv #(expand-expr env %) sources)
        vectors (filter vector? expanded-sources)]
    (when (empty? expanded-sources)
      (throw (ex-info "map expects an operation and at least one value source" {:args args})))
    (when (empty? vectors)
      (throw (ex-info "map expects at least one vector source" {:args args})))
    (let [lengths (map count vectors)
          n (first lengths)]
      (when-not (apply = lengths)
        (throw (ex-info "map vector sources must have the same length" {:args args})))
      (mapv (fn [idx]
              (let [values (map (fn [source]
                                  (if (vector? source)
                                    (source idx)
                                    source))
                                expanded-sources)]
                (expand-expr env (apply list op values))))
            (range n)))))

(defn validate-sample-data-cell! [value]
  (cond
    (number? value) true
    (note-symbol? value) true
    (symbol? value) (throw (ex-info (str "unknown symbol '" value "'") {:value value}))
    :else (throw (ex-info "expected number or note" {:value value}))))

(defn validate-sample-data-value! [value]
  (when-not (vector? value)
    (throw (ex-info "sample-data must be a vector" {:value value})))
  (when (empty? value)
    (throw (ex-info "sample-data requires at least one value" {:value value})))
  (doseq [cell value]
    (validate-sample-data-cell! cell))
  value)

(defn null-symbol? [value]
  (and (symbol? value)
       (#{"nil" "null"} (name value))))

(defn validate-sample-path-value! [value]
  (when-not (or (string? value) (null-symbol? value))
    (throw (ex-info "expected string" {:value value})))
  value)

(defn expand-sample-options [env options]
  (->> (partition 2 options)
       (mapcat (fn [[key value]]
                 (cond
                   (#{:sample-data :sample_data} key)
                   [key (validate-sample-data-value! (expand-expr env value))]

                   (#{:sample :sample-path :sample_path} key)
                   [key (validate-sample-path-value! (expand-expr env value))]

                   :else
                   [key value])))))

(defn expand-sample-form [env args]
  (let [[id sample-arg & more] args
        inline-options? (keyword? sample-arg)
        options (if inline-options?
                  (cons sample-arg more)
                  more)]
    (when-not id
      (throw (ex-info "sample requires a track id" {:args args})))
    (when-not (keyword? id)
      (throw (ex-info "sample track id must be a keyword" {:args args
                                                           :id id})))
    (when-not sample-arg
      (throw (ex-info "sample requires a wav path or :sample-data" {:args args})))
    (loop [remaining options]
      (when (seq remaining)
        (when-not (keyword? (first remaining))
          (throw (ex-info "sample options must be keyword/value pairs"
                          {:args args
                           :value (first remaining)})))
        (when-not (seq (rest remaining))
          (throw (ex-info (str "sample " (first remaining) " requires a value")
                          {:args args
                           :option (first remaining)})))
        (recur (nnext remaining))))
    (when-not inline-options?
      (validate-sample-path-value! (expand-expr env sample-arg)))
    (let [options (expand-sample-options env options)
          provided (set (take-nth 2 options))
          has-sample-source? (boolean (some provided [:sample-data :sample_data
                                                      :sample :sample-path :sample_path]))
          sample-path-items (when-not inline-options? [:sample-path sample-arg])
          defaults (mapcat identity
                           (remove #(contains? provided (first %))
                                   [[:note 'c3]
                                    [:gate 1]
                                    [:dur 1]
                                    [:amp 1]]))]
      (when (and inline-options? (not has-sample-source?))
        (throw (ex-info "sample requires a wav path or :sample-data" {:args args})))
      (expand-expr env
                   (apply list
                          'd id
                          :src :sample
                          (concat sample-path-items defaults options))))))

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
        arpeggio (generated-arpeggio @expanded-args)
        arp (generated-arpeggio @expanded-args)
        shape (generated-shape @expanded-args)
        transpose (let [[value semitones & extra] @expanded-args]
                    (when-not (= 2 (count @expanded-args))
                      (throw (ex-info "transpose expects value and semitones" {:args @expanded-args})))
                    (transpose-value value semitones))
        map (expand-map-form env args)
        with (expand-with-form env args)
        scene (expand-sectioned-scene-form env head args)
        block (expand-sectioned-scene-form env head args)
        section (throw (ex-info "section is only valid inside scene" {:args args}))
        p (expand-p-form env args)
        times (expand-times-form env args)
        then (expand-then-form env args)
        sample (expand-sample-form env args)
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
        (println "usage: ./glitchlisp-native compile <input.gl> <output.gl>"))
      (System/exit 2))
    (compile-file! input output)))
