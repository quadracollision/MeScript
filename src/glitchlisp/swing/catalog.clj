(ns glitchlisp.swing.catalog
  (:require [clojure.string :as str]
            [glitchlisp.swing.shared :as shared])
  (:import
    [javax.swing JComboBox]))

(def default-source "")

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
  {"Synth" "Synth"
   "Noise" "Noise"
   "Drum" "Drum"
   "Percussion" "Percussion"
   "Sample" "Sample"})

(defn oscillator-option-label [source]
  (str (get oscillator-source-group-labels (oscillator-source-group source) "Other")
       " / "
       source))

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
                     (map oscillator-option-label sources)))))))

(defn track-id-for-source [source]
  (-> source
      (str/replace #"-synth$" "")
      (str/replace #"[^A-Za-z0-9_-]" "-")))

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
  #{"sine-synth" "saw-synth" "square-synth" "tri-synth" "pulse" "morph"
    "supersaw" "wavetable" "fm-op" "additive" "sync" "pwm-sweep" "pad-wash"})

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
   ":sample-data" "type: vector<number|note>"
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
   (let [blank-param (fn [param]
                       (str "   " param " null"
                            (param-contract oscillator-param-contracts param include-comments?)
                            "\n"))]
     (if (= source "sample")
       (str "(sample :sample\n"
            (blank-param ":sample-data")
            (blank-param ":note")
            (blank-param ":dur")
            (blank-param ":amp")
            (blank-param ":gate")
            ")\n")
       (str "(d :" (track-id-for-source source) "\n"
            "   :src :" source (param-contract oscillator-param-contracts ":src" include-comments?) "\n"
            (blank-param ":note")
            (blank-param ":dur")
            (blank-param ":amp")
            (apply str (map blank-param (oscillator-parameter-examples source)))
            (blank-param ":gate")
            ")\n")))))

(defn oscillator-snippet [source]
  (let [{:keys [note gate dur amp extra]} (defaults-for-source source)
        id (track-id-for-source source)]
    (format "(d :%s\n   :src :%s\n   :note %s\n   :dur %s\n   :amp %s%s\n   :gate %s)\n"
            id source note dur amp extra gate)))

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

(def filter-type-contract
  "type: keyword; range: :lowpass|:lp|:highpass|:hp|:bandpass|:bp|:notch|:allpass|:ap|:peaking|:peak|:bell|:low-shelf|:low_shelf|:lowshelf|:high-shelf|:high_shelf|:highshelf")

(def distortion-type-contract
  "type: keyword; range: :tanh|:hard-clip|:hard_clip|:soft-clip|:soft_clip|:sine-fold|:sine_fold|:rectify|:half-rectify|:half_rectify|:waveshape")

(def effect-type-contracts
  {"filter" {":type" filter-type-contract}
   "distort" {":type" distortion-type-contract}
   "distortion" {":type" distortion-type-contract}
   "formant" {":vowel" "type: keyword; range: :a|:e|:i|:o|:u"}
   "bitcrush" {":bits" "type: integer; range: 2..16"
               ":bit-depth" "type: integer; range: 2..16"
               ":rate" "type: integer; range: 1..128"
               ":sample-rate-reduction" "type: integer; range: 1..128"}
   "crystal" {":brightness" "type: number; range: 0..1"
              ":decay" "type: number; range: 0..0.95"}
   "wavefolder" {":folds" "type: number; range: 1..8"
                 ":gain" "type: number; range: 0.1..12"
                 ":symmetry" "type: number; range: 0.1..2"}
   "fold" {":folds" "type: number; range: 1..8"
           ":gain" "type: number; range: 0.1..12"
           ":symmetry" "type: number; range: 0.1..2"}
   "resonator" {":freq" "type: number Hz; range: >=20"
                ":decay" "type: number; range: 0..1"
                ":harmonics" "type: number; range: 1..16"}
   "chorus" {":rate" "type: number Hz; range: >=0.01"
             ":depth" "type: number seconds; range: 0.0001..0.05"
             ":voices" "type: integer; range: 1..8"}
   "ensemble" {":rate" "type: number Hz; range: >=0.01"
               ":depth" "type: number seconds; range: 0.0005..0.05"
               ":voices" "type: integer; range: 2..12"}
   "ce1-chorus" {":rate" "type: number Hz; range: 0.01..10"
                 ":intensity" "type: number; range: 0..1"}
   "ce-1" {":rate" "type: number Hz; range: 0.01..10"
           ":intensity" "type: number; range: 0..1"}
   "re301-chorus" {":rate" "type: number Hz; range: 0.01..10"
                   ":depth" "type: number; range: 0..1"
                   ":tone" "type: number; range: 0..1"}
   "re-301-chorus" {":rate" "type: number Hz; range: 0.01..10"
                    ":depth" "type: number; range: 0..1"
                    ":tone" "type: number; range: 0..1"}
   "phaser" {":rate" "type: number Hz; range: 0.01..20"
             ":depth" "type: number; range: 0..1"
             ":stages" "type: integer; range: 1..12"}
   "dimension" {":mode" "type: integer; range: 1..4"}
   "dimension-d" {":mode" "type: integer; range: 1..4"}
   "flanger" {":rate" "type: number Hz; range: 0.01..20"
              ":depth" "type: number seconds; range: 0.0001..0.02"}
   "small-stone" {":rate" "type: number Hz; range: 0.01..20"
                  ":depth" "type: number; range: 0..1"
                  ":color" "type: boolean; range: true|false"}
   "vibrato" {":rate" "type: number Hz; range: >=0.01"
              ":depth" "type: number seconds; range: 0.0001..0.03"}
   "tremolo" {":rate" "type: number Hz; range: 0.01..40"
              ":depth" "type: number; range: 0..1"}
   "ring-mod" {":freq" "type: number Hz; range: 0.01..20000"}
   "ringmod" {":freq" "type: number Hz; range: 0.01..20000"}
   "arp-ring-mod" {":freq" "type: number Hz; range: 0.01..20000"
                   ":depth" "type: number; range: 0..1"
                   ":mix" "type: number; range: 0..1"
                   ":diode-curve" "type: number; range: 0..1"}
   "tube" {":drive" "type: number; range: 0..1"
           ":gain" "type: number; range: 0..1"
           ":asymmetry" "type: number; range: 0..1"}
   "tube-saturation" {":drive" "type: number; range: 0..1"
                      ":gain" "type: number; range: 0..1"
                      ":asymmetry" "type: number; range: 0..1"}
   "tape" {":saturation" "type: number; range: 0..1"
           ":input-level" "type: number; range: 0..1"
           ":wow" "type: number; range: 0..1"
           ":flutter" "type: number; range: 0..1"}
   "studer-tape" {":input-level" "type: number; range: 0..1"
                  ":speed" "type: number; range: 0..2"
                  ":bias" "type: number; range: 0..1"}
   "exciter" {":amount" "type: number; range: 0..1"}
   "fairchild" {":input-gain" "type: number; range: 0..1"
                ":time-constant" "type: number; range: 1..6"}
   "la2a" {":peak-reduction" "type: number; range: 0..1"
           ":mode" "type: keyword; range: :compress|:limit"}
   "1176" {":input-gain" "type: number; range: 0..1"
           ":attack" "type: number; range: 0..1"
           ":release" "type: number; range: 0..1"}
   "urei-1176" {":input-gain" "type: number; range: 0..1"
                ":attack" "type: number; range: 0..1"
                ":release" "type: number; range: 0..1"}
   "transient" {":attack-gain" "type: number; range: 0..8"
                ":sustain-gain" "type: number; range: 0..4"}
   "transient-shaper" {":attack-gain" "type: number; range: 0..8"
                       ":sustain-gain" "type: number; range: 0..4"}
   "reverb" {":decay" "type: number; range: 0..1"}
   "spring-reverb" {":decay" "type: number; range: 0..4"
                    ":tone" "type: number; range: 0..1"
                    ":drip" "type: number; range: 0..1"}
   "emt-plate" {":decay" "type: number; range: 0.1..5"
                ":damping" "type: number; range: 0..1"}
   "lexicon-224" {":size" "type: number; range: 0.2..2"
                  ":decay" "type: number; range: 0.1..8"
                  ":damping" "type: number; range: 0..1"}
   "ams-reverb" {":decay" "type: number; range: 0.1..5"
                 ":damping" "type: number; range: 0..1"
                 ":program" "type: keyword; range: :nonlin|:non-linear|:nonlinear|:ambience|:ambient|:plate"}
   "moog" {":drive" "type: number; range: 0..1"}
   "moog-ladder" {":drive" "type: number; range: 0..1"}
   "tb-303" {":env-mod" "type: number; range: 0..1"
             ":accent" "type: number; range: 0..1"}
   "tb303" {":env-mod" "type: number; range: 0..1"
            ":accent" "type: number; range: 0..1"}
   "buchla-lpg" {":strike" "type: number; range: 0..1"}
   "lpg" {":strike" "type: number; range: 0..1"}
   "neve-preamp" {":gain" "type: number; range: 0..1"
                  ":warmth" "type: number; range: 0..1"}
   "marshall-amp" {":gain" "type: number; range: 0..1"
                   ":tone" "type: number; range: 0..1"
                   ":presence" "type: number; range: 0..1"}
   "vox-ac30" {":gain" "type: number; range: 0..1"
               ":treble" "type: number; range: 0..1"
               ":cut" "type: number; range: 0..1"}
   "fender-twin" {":volume" "type: number; range: 0..1"
                  ":gain" "type: number; range: 0..1"
                  ":treble" "type: number; range: 0..1"
                  ":bass" "type: number; range: 0..1"
                  ":reverb-mix" "type: number; range: 0..1"}
   "pultec-eq" {":low-boost" "type: number; range: 0..1"
                ":low-atten" "type: number; range: 0..1"
                ":high-boost" "type: number; range: 0..1"
                ":high-atten" "type: number; range: 0..1"}
   "pultec" {":low-boost" "type: number; range: 0..1"
             ":low-atten" "type: number; range: 0..1"
             ":high-boost" "type: number; range: 0..1"
             ":high-atten" "type: number; range: 0..1"}
   "space-echo" {":time" "type: number seconds; range: 0.02..2"
                 ":wow" "type: number; range: 0..1"
                 ":flutter" "type: number; range: 0..1"
                 ":tone" "type: number; range: 0..1"
                 ":spring-mix" "type: number; range: 0..1"}
   "re201" {":time" "type: number seconds; range: 0.02..2"
            ":wow" "type: number; range: 0..1"
            ":flutter" "type: number; range: 0..1"
            ":tone" "type: number; range: 0..1"
            ":spring-mix" "type: number; range: 0..1"}
   "re-201" {":time" "type: number seconds; range: 0.02..2"
             ":wow" "type: number; range: 0..1"
             ":flutter" "type: number; range: 0..1"
             ":tone" "type: number; range: 0..1"
             ":spring-mix" "type: number; range: 0..1"}
   "tc2290" {":time-ms" "type: number ms; range: 1..2000"
             ":mod-rate" "type: number Hz; range: 0..20"
             ":mod-depth" "type: number seconds; range: 0..0.05"}
   "tc-2290" {":time-ms" "type: number ms; range: 1..2000"
              ":mod-rate" "type: number Hz; range: 0..20"
              ":mod-depth" "type: number seconds; range: 0..0.05"}
   "stutter" {":grain-size-ms" "type: number ms; range: 1..500"
              ":grain-ms" "type: number ms; range: 1..500"
              ":repeats" "type: integer; range: 1..16"}
   "granular-stutter" {":grain-size-ms" "type: number ms; range: 1..500"
                       ":grain-ms" "type: number ms; range: 1..500"
                       ":repeats" "type: integer; range: 1..16"}
   "glitch" {":density" "type: number; range: 0..1"
             ":slice-ms" "type: number ms; range: 1..500"}
   "fade" {":fade-in-ms" "type: number ms; range: >=0"
           ":fade-out-ms" "type: number ms; range: >=0"
           ":duration" "type: number seconds; range: >=0.001"}
   "adsr" {":attack" "type: number seconds; range: >=0"
           ":a" "type: number seconds; range: >=0"
           ":decay" "type: number seconds; range: >=0"
           ":d" "type: number seconds; range: >=0"
           ":sustain" "type: number; range: 0..1"
           ":s" "type: number; range: 0..1"
           ":release" "type: number seconds; range: >=0"
           ":r" "type: number seconds; range: >=0"
           ":duration" "type: number seconds; range: >=0.001"}
   "asdr" {":attack" "type: number seconds; range: >=0"
           ":a" "type: number seconds; range: >=0"
           ":decay" "type: number seconds; range: >=0"
           ":d" "type: number seconds; range: >=0"
           ":sustain" "type: number; range: 0..1"
           ":s" "type: number; range: 0..1"
           ":release" "type: number seconds; range: >=0"
           ":r" "type: number seconds; range: >=0"
           ":duration" "type: number seconds; range: >=0.001"}
   "doppler" {":speed" "type: number; range: 0.01..8"
              ":depth" "type: number; range: 0..1"}
   "tape-stop" {":duration" "type: number; range: 0.1..1"
                ":duration-pct" "type: number; range: 0.1..1"}
   "sem-filter" {":type" filter-type-contract}
   "sem" {":type" filter-type-contract}
   "obxa-filter" {":type" filter-type-contract}
   "haas" {":side" "type: keyword; range: :left|:right"}
   })

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
   ":feedback" "type: number; range: 0..0.95"
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
     (:form (effect-option-for-label label))
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
  (oscillator-option-source (.getSelectedItem combo)))

(defn selected-effect-form [^JComboBox combo]
  (let [label (str (.getSelectedItem combo))]
    (:form (effect-option-for-label label))))
