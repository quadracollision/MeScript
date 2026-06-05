(ns glitchlisp.swing.catalog
  (:require [clojure.string :as str]
            [glitchlisp.swing.shared :as shared])
  (:import
    [javax.swing JComboBox]))

(def default-source
  "")

(defn load-oscillators []
  (try
    (read-string (shared/file-or-resource-slurp "data/oscillators.edn"))
    (catch Exception _
      ["sine-synth" "saw-synth" "square-synth" "kick-808" "snare-808" "hat-909"])))

(def oscillator-sources
  (load-oscillators))

(def drum-sources
  #{"kick-synth" "snare" "hat" "kick-808" "snare-808" "hat-808" "cowbell-808"
    "kick-909" "snare-909" "hat-909" "kick-78" "snare-78" "hat-78"
    "kick-707" "snare-707"})

(def percussion-sources
  #{"click" "clap" "cymbal-crash" "cymbal-ride" "tom" "rimshot" "shaker"
    "woodblock" "cowbell" "zap" "scratch" "impact"})

(def noise-sources
  #{"noise-synth" "noise-white" "noise-pink" "noise-brown" "noise-blue"
    "noise-purple"})

(def sample-sources
  #{"sample"})

(defn oscillator-source-group [source]
  (cond
    (contains? drum-sources source) "Drum"
    (contains? percussion-sources source) "Percussion"
    (contains? noise-sources source) "Noise"
    (contains? sample-sources source) "Sample"
    :else "Synth"))

(def oscillator-source-group-order
  {"Synth" 0
   "Noise" 1
   "Drum" 2
   "Percussion" 3
   "Sample" 4})

(def oscillator-source-group-labels
  {"Synth" "/Synths"
   "Noise" "/Noise"
   "Drum" "/Drums"
   "Percussion" "/Percussion"
   "Sample" "/Sample"})

(defn oscillator-option-label [source]
  source)

(defn oscillator-option-header? [option]
  (str/starts-with? (str option) "/"))

(defn oscillator-option-source [option]
  (let [option (str option)]
    (cond
      (oscillator-option-header? option) nil
      (re-matches #"[^/]+ /\s*(.+)" option) (second (re-matches #"[^/]+ /\s*(.+)" option))
      :else option)))

(defn oscillator-option-labels []
  (let [grouped (->> oscillator-sources
                     (group-by oscillator-source-group))]
    (->> oscillator-source-group-order
         (sort-by val)
         (mapcat (fn [[group _]]
                   (when-let [sources (seq (sort (get grouped group)))]
                     (cons (get oscillator-source-group-labels group)
                           (map oscillator-option-label sources))))))))

(defn track-id-for-source [source]
  (cond
    (str/includes? source "kick") "kick"
    (str/includes? source "snare") "snare"
    (or (= source "hat") (str/includes? source "hat")) "hat"
    (some #{source} ["cowbell" "cowbell-808" "woodblock" "rimshot" "shaker" "clap" "tom"]) "perc"
    (some #{source} ["fm-op" "pluck" "bass-slap"]) "bass"
    (some #{source} ["pad-wash" "strings" "brass" "organ" "drone-dark"]) "pad"
    (some #{source} ["bell" "glass" "vocal" "breath"]) "tone"
    (str/includes? source "noise") "noise"
    :else "lead"))

(defn defaults-for-source [source]
  (cond
    (str/includes? source "kick")
    {:note "c1" :gate "(p [1 0 0 0 1 0 0 0])" :dur "0.36" :amp "0.42" :extra ""}

    (str/includes? source "snare")
    {:note "c3" :gate "(p [0 0 0 0 1 0 0 0])" :dur "0.16" :amp "0.22" :extra ""}

    (or (= source "hat") (str/includes? source "hat"))
    {:note "c6" :gate "(p [0 1 0 1 0 1 0 1])" :dur "0.03" :amp "0.06" :extra ""}

    (some #{source} ["supersaw" "sync" "saw-synth" "square-synth" "pulse"])
    {:note "(p [c4 eb4 g4 bb4])" :gate "(p [1 0 1 1 0 1 0 1])" :dur "0.08" :amp "0.12"
     :extra "\n   :unison 5\n   :unison-detune 9\n   :unison-spread 0.8"}

    (= source "fm-op")
    {:note "(p [c2 c2 eb2 g1 bb1 c2])" :gate "(p [1 0 1 1 0 1 0 1])" :dur "0.16" :amp "0.22"
     :extra "\n   :fm-ratio 1.5\n   :fm-depth 2.2"}

    (some #{source} ["pad-wash" "strings" "brass" "organ" "drone-dark"])
    {:note "(p [c2 eb2 g2 bb2])" :gate "(p [1 0 0 0 0 0 0 0])" :dur "1.8" :amp "0.12"
     :extra "\n   :unison 5\n   :unison-detune 10\n   :unison-spread 0.8"}

    :else
    {:note "(p [c3 eb3 g3 bb3])" :gate "(p [1 0 1 0 0 1 0 1])" :dur "0.12" :amp "0.16" :extra ""}))

(def detune-capable-sources
  #{"sine-synth" "saw-synth" "tri-synth" "supersaw" "additive" "pad-wash"})

(def phase-capable-sources
  #{"sine-synth" "saw-synth" "square-synth" "tri-synth" "pulse" "morph"
    "supersaw" "wavetable" "fm-op" "additive" "sync" "pwm-sweep" "harsh"
    "chip" "strings" "brass" "organ" "bell" "glass" "vocal" "breath"
    "pad-wash" "click"})

(def pulse-width-capable-sources
  #{"square-synth" "pulse"})

(def morph-capable-sources
  #{"morph" "supersaw" "wavetable" "vocal"})

(def fm-ratio-capable-sources
  #{"fm-op" "sync" "pwm-sweep"})

(def fm-depth-capable-sources
  #{"fm-op" "bell"})

(def harmonics-capable-sources
  #{"additive"})

(def unison-capable-sources
  (disj (set oscillator-sources) "sample"))

(defn oscillator-parameter-examples [source]
  (cond-> []
    (contains? detune-capable-sources source) (conj ":detune-cents")
    (contains? phase-capable-sources source) (conj ":phase")
    (contains? pulse-width-capable-sources source) (conj ":pulse-width")
    (contains? morph-capable-sources source) (conj ":morph")
    true (conj ":gain")
    (contains? unison-capable-sources source) (conj ":unison" ":unison-detune" ":unison-spread")
    (contains? fm-ratio-capable-sources source) (conj ":fm-ratio")
    (contains? fm-depth-capable-sources source) (conj ":fm-depth")
    (contains? harmonics-capable-sources source) (conj ":harmonics")))

(def oscillator-param-contracts
  {":src" "type: oscillator keyword"
   ":note" "type: note or note-pattern"
   ":gate" "type: gate-pattern; range: 0 rest, 1 hit, nested subdivision, hold suffix"
   ":dur" "type: number seconds or number-pattern; range: 0.005..4"
   ":amp" "type: number or number-pattern; range: 0..1"
   ":detune-cents" "type: number cents or number-pattern; range: any"
   ":phase" "type: number cycles or number-pattern; range: wrapped to 0..<1"
   ":pulse-width" "type: number or number-pattern; range: 0.01..0.99"
   ":morph" "type: number or number-pattern; range: 0..1"
   ":gain" "type: number or number-pattern; range: 0..2"
   ":unison" "type: integer or integer-pattern; range: 1..10"
   ":unison-detune" "type: number cents or number-pattern; range: 0..100"
   ":unison-spread" "type: number or number-pattern; range: 0..1"
   ":fm-ratio" "type: number or number-pattern; range: >=0.01"
   ":fm-depth" "type: number or number-pattern; range: 0..32"
   ":harmonics" "type: vector<number>; range: 0..2 each, max 8 values"})

(defn param-contract
  ([contracts param]
   (param-contract contracts param true))
  ([contracts param include-comments?]
   (if include-comments?
     (str " ; " (get contracts param "type: value; range: accepted by parser"))
     "")))

(defn oscillator-structure-snippet
  ([source]
   (oscillator-structure-snippet source true))
  ([source include-comments?]
   (str "(d :" (track-id-for-source source) "\n"
        "   :src :" source (param-contract oscillator-param-contracts ":src" include-comments?) "\n"
        "   :note (p [])" (param-contract oscillator-param-contracts ":note" include-comments?) "\n"
        "   :gate (p [])" (param-contract oscillator-param-contracts ":gate" include-comments?) "\n"
        "   :dur null" (param-contract oscillator-param-contracts ":dur" include-comments?) "\n"
        "   :amp null" (param-contract oscillator-param-contracts ":amp" include-comments?) "\n"
        (apply str (map (fn [param]
                          (str "   " param " null" (param-contract oscillator-param-contracts param include-comments?) "\n"))
                        (oscillator-parameter-examples source)))
        ")\n")))

(defn oscillator-snippet [source]
  (let [{:keys [note gate dur amp extra]} (defaults-for-source source)
        id (track-id-for-source source)]
    (format "(d :%s\n   :src :%s\n   :note %s\n   :gate %s\n   :dur %s\n   :amp %s%s)\n"
            id source note gate dur amp extra)))

(defn load-effects []
  (try
    (read-string (shared/file-or-resource-slurp "data/effects.edn"))
    (catch Exception _
      [{:label "FX Vector"
        :form ":fx [(filter :type :lowpass :cutoff 1200 :res 0.35)\n     (delay :time 0.125 :feedback 0.32 :mix 0.22)]"}
       {:label "filter" :form "(filter :type :lowpass :cutoff 1200 :res 0.35)"}
       {:label "delay" :form "(delay :time 0.125 :feedback 0.32 :mix 0.22)"}])))

(def effect-options
  (load-effects))

(defn effect-option-for-label [label]
  (some #(when (= (:label %) label) %) effect-options))

(def effect-type-contracts
  {"filter" {":type" "type: keyword; range: :lowpass|:highpass|:bandpass|:notch|:allpass|:peaking|:low-shelf|:high-shelf"}
   "distort" {":type" "type: keyword; range: :tanh|:hard-clip|:soft-clip|:sine-fold|:rectify|:half-rectify|:waveshape"}
   "distortion" {":type" "type: keyword; range: :tanh|:hard-clip|:soft-clip|:sine-fold|:rectify|:half-rectify|:waveshape"}
   "formant" {":vowel" "type: keyword; range: :a|:e|:i|:o|:u"}
   "haas" {":side" "type: keyword; range: :left|:right"}
   "ams-reverb" {":program" "type: keyword; range: :nonlin|:ambience|:plate"}
   "small-stone" {":color" "type: boolean; range: true|false"}
   "la2a" {":mode" "type: keyword; range: :compress|:limit"}})

(def effect-param-contracts
  {":type" "type: keyword"
   ":vowel" "type: keyword"
   ":side" "type: keyword"
   ":program" "type: keyword"
   ":color" "type: boolean; range: true|false"
   ":bits" "type: integer; range: >=1"
   ":voices" "type: integer; range: >=1"
   ":mode" "type: integer or keyword; range: effect-specific"
   ":repeats" "type: integer; range: >=0"
   ":harmonics" "type: integer; range: >=1"
   ":mix" "type: number; range: 0..1"
   ":depth" "type: number; range: 0..1"
   ":feedback" "type: number; range: 0..1"
   ":res" "type: number; range: 0..1"
   ":resonance" "type: number; range: 0..1"
   ":width" "type: number; range: >=0"
   ":amount" "type: number; range: 0..1"
   ":density" "type: number; range: 0..1"
   ":quality" "type: number; range: 0..1"
   ":intensity" "type: number; range: 0..1"
   ":brightness" "type: number; range: 0..1"
   ":crackle" "type: number; range: 0..1"
   ":hiss" "type: number; range: 0..1"
   ":wow" "type: number; range: 0..1"
   ":air" "type: number; range: 0..1"
   ":warmth" "type: number; range: 0..1"
   ":tone" "type: number; range: 0..1"
   ":position" "type: number; range: 0..1"
   ":height" "type: number; range: 0..1"
   ":freeze" "type: number; range: 0..1"
   ":drive" "type: number; range: >=0"
   ":gain" "type: number; range: >=0"
   ":gain-db" "type: number dB; range: any"
   ":cutoff" "type: number Hz; range: >0"
   ":freq" "type: number Hz; range: >0"
   ":rate" "type: number Hz; range: >=0"
   ":time" "type: number seconds; range: >=0"
   ":time-ms" "type: number ms; range: >=0"
   ":delay" "type: number ms; range: >=0"
   ":delay-ms" "type: number ms; range: >=0"
   ":grain-ms" "type: number ms; range: >0"
   ":slice-ms" "type: number ms; range: >0"
   ":duration" "type: number seconds; range: >=0"
   ":duration-pct" "type: number; range: 0..1"
   ":decay" "type: number; range: >=0"
   ":attack" "type: number seconds; range: >=0"
   ":release" "type: number seconds; range: >=0"
   ":attack-ms" "type: number ms; range: >=0"
   ":release-ms" "type: number ms; range: >=0"
   ":threshold" "type: number dB; range: any"
   ":ratio" "type: number; range: >0"
   ":makeup" "type: number dB; range: any"
   ":makeup-db" "type: number dB; range: any"
   ":ceiling" "type: number dB; range: <=0 typical"
   ":semitones" "type: number semitones; range: any"
   ":shift-semitones" "type: number semitones; range: any"
   ":shift-hz" "type: number Hz; range: any"
   ":detune-cents" "type: number cents; range: any"
   ":interval" "type: number semitones; range: any"
   ":octave-up" "type: number; range: 0..1"
   ":octave-down" "type: number; range: 0..1"
   ":low-thresh" "type: number dB; range: any"
   ":mid-thresh" "type: number dB; range: any"
   ":high-thresh" "type: number dB; range: any"
   ":crossover-low" "type: number Hz; range: >0"
   ":crossover-high" "type: number Hz; range: >0"
   ":low-harmonics" "type: number; range: 0..1"
   ":high-harmonics" "type: number; range: 0..1"
   ":room-size" "type: number; range: 0..1"
   ":bass-mono" "type: number Hz; range: >=0"
   ":pre-delay-ms" "type: number ms; range: >=0"
   ":damping" "type: number; range: 0..1"
   ":drip" "type: number; range: 0..1"
   ":asymmetry" "type: number; range: 0..1"
   ":treble" "type: number; range: 0..1"
   ":bass" "type: number; range: 0..1"
   ":presence" "type: number; range: 0..1"
   ":volume" "type: number; range: 0..1"
   ":reverb-mix" "type: number; range: 0..1"
   ":low-boost" "type: number; range: 0..1"
   ":high-boost" "type: number; range: 0..1"
   ":bit-depth" "type: integer; range: >=1"
   ":input-gain" "type: number; range: >=0"
   ":peak-reduction" "type: number; range: 0..1"
   ":attack-gain" "type: number; range: >=0"
   ":sustain-gain" "type: number; range: >=0"
   ":sensitivity" "type: number; range: >=0"
   ":folds" "type: number; range: >=0"
   ":symmetry" "type: number; range: 0..1"
   ":size" "type: number; range: 0..1"
   ":pitch" "type: number ratio; range: >0"
   ":rate-ms" "type: number ms; range: >=0"
   ":diode-curve" "type: number; range: 0..1"
   ":flutter" "type: number; range: 0..1"
   ":saturation" "type: number; range: 0..1"
   ":cut" "type: number; range: 0..1"})

(defn effect-param-contract [effect-name param]
  (or (get-in effect-type-contracts [effect-name param])
      (get effect-param-contracts param)
      "type: number; range: accepted by effect"))

(defn blank-effect-form
  ([label]
   (blank-effect-form label true))
  ([label include-comments?]
   (if (= label "FX Vector")
     ":fx []"
     (let [form (or (:form (effect-option-for-label label)) (str "(" label ")"))
           trimmed (str/trim form)
           inner (if (and (str/starts-with? trimmed "(") (str/ends-with? trimmed ")"))
                   (subs trimmed 1 (dec (count trimmed)))
                   trimmed)
           tokens (str/split inner #"\s+")
           effect-name (first tokens)
           params (->> (rest tokens)
                       (partition-all 2)
                       (keep (fn [[param example]]
                               (when (and param (str/starts-with? param ":"))
                                 [param example]))))]
       (if (seq params)
         (let [guide (when include-comments?
                       (->> params
                             (map (fn [[param _]]
                                    (str param " " (effect-param-contract effect-name param))))
                             (str/join ", ")))]
           (str "(" effect-name
                " "
                (str/join " " (map (fn [[param _]]
                                      (str param " null"))
                                    params))
                ")"
                (when guide (str " ; " guide))))
         (str "(" effect-name ")"))))))

(defn selected-oscillator-source [^JComboBox combo]
  (nth oscillator-sources (.getSelectedIndex combo)))

(defn selected-effect-form [^JComboBox combo]
  (:form (nth effect-options (.getSelectedIndex combo))))
