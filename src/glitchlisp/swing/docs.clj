(ns glitchlisp.swing.docs
  (:require [clojure.string :as str]
            [glitchlisp.swing.catalog :as catalog])
  (:import
    [java.awt BorderLayout Dimension Font]
    [java.awt.event ActionListener]
    [javax.swing JButton JFrame JPanel JScrollPane JTextField JTextPane]
    [javax.swing.event HyperlinkEvent$EventType HyperlinkListener]))

(defn html-escape [value]
  (-> (str value)
      (str/replace "&" "&amp;")
      (str/replace "<" "&lt;")
      (str/replace ">" "&gt;")
      (str/replace "\"" "&quot;")))

(defn slug [value]
  (let [text (-> value str str/lower-case
                 (str/replace #"[^a-z0-9!]+" "-")
                 (str/replace #"^-|-$" ""))]
    (if (str/blank? text) "entry" text)))

(defn entry-id [section-id name]
  (str section-id "-" (slug name)))

(defn entry-html [section-id [name description example]]
  (let [id (entry-id section-id name)]
    (str "<div class='entry'>"
         "<a name='" id "'></a>"
         "<div><code>" (html-escape name) "</code></div>"
         "<p>" (html-escape description) "</p>"
         "<pre>" (html-escape example) "</pre>"
         "</div>")))

(defn section-html [collapsed-section-ids {:keys [id title rows]}]
  (str "<section>"
       "<a name='" id "'></a>"
       "<h2><a href='#toggle:" id "'>"
       (if (contains? collapsed-section-ids id) "+ " "- ")
       (html-escape title)
       "</a></h2>"
       (when-not (contains? collapsed-section-ids id)
         (apply str (map #(entry-html id %) rows)))
       "</section>"))

(defn toc-html [sections]
  (str "<nav>"
       "<h2>Contents</h2>"
       "<ul>"
       (apply str
              (map (fn [{:keys [id title]}]
                     (str "<li><a href='#" id "'>" (html-escape title) "</a></li>"))
                   sections))
       "</ul>"
       "</nav>"))

(defn index-html [sections]
  (str "<section>"
       "<a name='index'></a>"
       "<h2>Index</h2>"
       "<div class='index'>"
       (->> sections
            (mapcat (fn [{:keys [id rows]}]
                      (map (fn [[name]]
                             (str "<a href='#" (entry-id id name) "'>"
                                  (html-escape name)
                                  "</a>"))
                           rows)))
            (str/join " "))
       "</div>"
       "</section>"))

(def top-level-forms
  [["(bpm N)" "Set tempo from 20 to 320 BPM." "(bpm 120)"]
   ["(d :id ...)" "Define a playable pattern." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)"]
   ["(sample :id SOURCE ...)" "Define a sample track from a wav file or inline data." "(sample :hit :sample-data [0 1 0 -1] :gate (p [1 0 0 0]))\n(start!)"]
   ["(include \"file.gl\")" "Load another .gl file before compiling; relative paths resolve from the including file." "(include \"examples/include-parts/drums.gl\")\n(scene :intro :loop true kick hat)\n(play-scene :intro)"]
   ["(scene :name ...)" "Define a scene." "(scene :intro :loop true (d :lead :src :sine-synth :gate 1))\n(play-scene :intro)"]
   ["(play-scene :name)" "Start a scene." "(scene :intro :loop true (d :lead :src :sine-synth :gate 1))\n(play-scene :intro)"]
   ["(start!)" "Start top-level tracks." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)"]
   ["(stop!)" "Stop playback." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(stop!)"]
   ["(play-note NOTE)" "Play one sine note." "(play-note c3)"]
   ["(post-fx [...])" "Apply render/master effects to an audio source." "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [(reverb :mix 0.2)])\n(start!)"]
   ["(mute :id)" "Mute a track." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(mute :lead)"]
   ["(unmute :id)" "Unmute a track." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(mute :lead)\n(unmute :lead)"]
   ["(solo :id)" "Solo a track." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(solo :lead)"]
   ["(unsolo :id)" "Unsolo a track." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(solo :lead)\n(unsolo :lead)"]
   ["(clear :id)" "Remove a track." "(d :lead :src :sine-synth :note c3 :gate 1)\n(d :keep :src :sine-synth :note e3 :gate 1)\n(start!)\n(clear :lead)"]
   ["(clear-all)" "Clear runtime state." "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(clear-all)"]])

(def compatibility-aliases
  [["(block :name ...)" "Compatibility alias for scene; prefer scene in new code." "(block :intro :loop true (d :lead :src :sine-synth :gate 1))\n(play-scene :intro)"]
   ["(play-block :name)" "Compatibility alias for play-scene; prefer play-scene in new code." "(block :intro :loop true (d :lead :src :sine-synth :gate 1))\n(play-block :intro)"]
   ["(cue :name)" "Short live alias for play-scene." "(scene :intro :loop true (d :lead :src :sine-synth :gate 1))\n(cue :intro)"]
   ["(master-fx [...])" "Compatibility alias for post-fx; prefer post-fx in new code." "(d :lead :src :sine-synth :note c3 :gate 1)\n(master-fx [(tape :saturation 0.4)])\n(start!)"]
   [":repeats N" "Compatibility alias for scene :repeat." "(scene :a :repeats 2 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":times N" "Compatibility alias for scene :repeat; prefer :repeat to avoid confusing it with pattern times." "(scene :a :times 2 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":length N" "Compatibility alias for scene :steps." "(scene :a :length 16 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":length-of :id" "Compatibility alias for scene :steps-of." "(scene :a :length-of :kick (d :kick :src :kick-synth :gate (p [1 0 0 0])))\n(play-scene :a)"]
   [":detune" "Compatibility alias for :detune-cents." "(d :lead :src :sine-synth :note c3 :gate 1 :detune 7)\n(start!)"]
   [":pw" "Compatibility alias for :pulse-width." "(d :lead :src :pulse :note c3 :gate 1 :pw 0.4)\n(start!)"]
   [":sample" "Compatibility alias for :sample-path; requires an existing wav file." "(d :hit :src :sample :sample \"kick.wav\" :gate 1)"]
   ["nil" "Compatibility alias for null." "(d :lead :src :sine-synth :note c3 :gate 1 :dur nil)\n(start!)"]
   [":resonance" "Compatibility alias for effect :res." ":fx [(filter :resonance 0.45)]"]
   [":gain_db" "Compatibility alias for effect :gain-db." ":fx [(filter :type :peaking :cutoff 900 :gain_db 3)]"]
   [":bit-depth" "Compatibility alias for bitcrush :bits." ":fx [(bitcrush :bit-depth 8)]"]
   [":sample-rate-reduction" "Compatibility alias for bitcrush :rate." ":fx [(bitcrush :sample-rate-reduction 4)]"]
   [":duration-pct" "Compatibility alias for tape-stop :duration." "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [(tape-stop :duration-pct 0.5)])\n(start!)"]
   [":delay" "Compatibility alias for comb :delay-ms." ":fx [(comb :delay 8)]"]
   ["(g [...])" "Short alias for gate-seq; prefer gate-seq in new code." ":note (g [c3 e3])"]
   ["(gs [...])" "Short alias for gate-seq; prefer gate-seq in new code." ":note (gs [c3 e3])"]
   ["(gate_seq [...])" "Compatibility alias for gate-seq; prefer gate-seq in new code." ":note (gate_seq [c3 e3])"]
   ["(rev X)" "Compatibility alias for reverse; prefer reverse in new code." ":gate (rev (p [1 0]))"]
   ["(arp root kind)" "Compatibility alias for arpeggio; prefer arpeggio in new code." ":note (p (arp c3 :minor7))"]])

(def syntax-basics
  [["; comment" "Comment to end of line." "; drums"]
   [":keyword" "Name used as an id or option." ":intro"]
   ["[...]" "Vector of values." "[1 0 1 0]"]
   ["(...)" "Form call." "(bpm 120)"]
   ["NOTE" "Pitch name; sharps and flats are supported." "c3 eb3 f#4"]
   ["NUMBER" "Numeric value." "0.25"]
   ["STRING" "Text value." "\"kick.wav\""]
   ["null" "Leave default value." ":amp null"]])

(def quick-start
  [["Runnable starter" "Copy this into a blank workstation buffer to hear the smallest scene-based sketch."
    "(bpm 100)

(def click
  (d :click
     :src :click
     :note (p [e4])
     :gate (p [1 0 0 0])
     :dur 0.05
     :amp 0.6))

(scene :intro :loop true
  click)

(play-scene :intro)"]
   ["Reusable track in a scene" "Canonical def / scene / play-scene shape for arranging reusable tracks."
    "(def lead
  (d :lead
     :src :sine-synth
     :note c3
     :gate (p [1 0 1 0])
     :dur 0.12
     :amp 0.2))

(scene :intro :loop true
  lead)

(play-scene :intro)"]
   ["Counted gate changes" "Use then / times when one track changes gate pattern over time."
    "(d :kick
   :src :kick-synth
   :note c2
   :gate (p (then
             (times 2 [1 0 0 0])
             [1 1 1 1])))

(start!)"]])

(def scene-options
  [[":loop true" "Loop this scene forever; true-only alias for repeat 0." "(scene :a :loop true (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":repeat N" "Repeat count; 0 loops forever." "(scene :a :repeat 2 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":steps N" "Set positive scene length in steps." "(scene :a :steps 16 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":bars N" "Set positive length in bars; defaults to 16 steps per bar." "(scene :a :bars 2 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":bar-steps N" "Set steps per bar for :bars." "(scene :a :bars 2 :bar-steps 8 (d :lead :src :sine-synth :gate 1))\n(play-scene :a)"]
   [":bar-steps-of :id" "Use a track cycle as the bar length for :bars." "(scene :a :bars 2 :bar-steps-of :kick (d :kick :src :kick-synth :gate (p [1 0 0 0])))\n(play-scene :a)"]
   [":steps-of :id" "Use a track's cycle length." "(scene :a :steps-of :kick (d :kick :src :kick-synth :gate (p [1 0 0 0])))\n(play-scene :a)"]
   [":loop-by :id N" "Use N cycles of a track as the scene length." "(scene :a :loop-by :kick 4 (d :kick :src :kick-synth :gate (p [1 0 0 0])))\n(play-scene :a)"]
   [":next :scene" "Move to another scene." "(scene :a :repeat 1 :next :b (d :lead :src :sine-synth :gate 1))\n(scene :b :loop true (d :lead :src :sine-synth :note e3 :gate 1))\n(play-scene :a)"]])

(def track-params
  [[":src" "Oscillator or sample source." ":src :additive"]
   [":note" "Note, chord, or note pattern." ":note (p [c3 e3 g3])"]
   [":gate" "Hit/rest pattern." ":gate (p [1 0 1 0])"]
   [":dur" "Voice duration in seconds from 0.005 to 4." ":dur 0.2"]
   [":amp" "Track amplitude from 0 to 1." ":amp 0.4"]
   [":fx" "Track effect chain." ":fx [(delay :time 0.125)]"]
   [":every" "Play every positive N local steps." ":every 2"]
   [":offset" "Offset track step index." ":offset 1"]
   [":detune-cents" "Detune in cents." ":detune-cents 7"]
   [":phase" "Oscillator phase." ":phase 0.25"]
   [":pulse-width" "Pulse width from 0.01 to 0.99." ":pulse-width 0.4"]
   [":morph" "Morph position from 0 to 1." ":morph 0.5"]
   [":gain" "Oscillator gain from 0 to 2." ":gain 1.2"]
   [":unison" "Number of unison voices from 1 to 10." ":unison 4"]
   [":unison-detune" "Unison detune cents from 0 to 100." ":unison-detune 8"]
   [":unison-spread" "Stereo unison width from 0 to 1." ":unison-spread 0.7"]
   [":fm-ratio" "FM/sync ratio of at least 0.01." ":fm-ratio 2"]
   [":fm-depth" "FM depth from 0 to 32." ":fm-depth 3"]
   [":harmonics" "Up to 8 additive harmonic levels from 0 to 2." ":harmonics [1 0.5 0.25]"]
   [":sample-path" "Load a wav file." ":sample-path \"kick.wav\""]
   [":sample-data" "Inline sample values." ":sample-data [0 1 0 -1]"]])

(def pattern-forms
  [["(p [...])" "Step pattern." "(p [1 0 1 0])"]
   ["(s [...])" "Hit/slot pattern alias accepted by notes, numeric params, gates, and on :gate." "(s [1 0 1 0])"]
   ["(g [...])" "Gate-slot pattern alias accepted by notes, numeric params, gates, and on :gate." "(g [1 0 1 0])"]
   ["(p :repeat N [...])" "Repeat a pattern." "(p :repeat 2 [1 0])"]
   ["(then A B ...)" "Play pattern stages in order for gates, notes, and numeric parameter patterns. A final plain gate pattern loops; a final gate times stage loops the full gate chain." "(p (then (times 2 [0 0 0 1]) [1 0 1 0]))"]
   ["(times N PATTERN)" "Repeat a pattern N times as a counted stage inside then." "(p (then (times 2 [1 0 0 0]) [1 1 1 1]))"]
   ["(p [[...]])" "Nested note values play together as a chord step." "(p [[c3 eb3 g3]])"]
   [":note [...]" "Bare note arrays advance on gate hits." ":note [c3 d3 e3 f3]"]
   ["(s [notes])" "Advance notes on hits." "(s [c3 e3 g3])"]
   ["(gate-seq [...])" "Advance notes on gate slots." "(gate-seq [c3 e3 g3])"]
   ["(euclid P S)" "Euclidean gate pattern." "(euclid 5 16)"]
   ["(euclid-rot P S R)" "Rotated Euclidean pattern." "(euclid-rot 5 16 2)"]
   ["(gate-hold N)" "Extend a hit by N slots." "(p [1 (gate-hold 2) 1])"]
   ["1_N" "Short gate hold; later hits in the held span can still play." "(p [1 1_2 1 1])"]
   ["Nested gates" "Subdivide a step." "(p [[1 1] 0 1])"]])

(def effect-forms
  [[":fx [...]" "Track effect chain." ":fx [(delay :time 0.125)]"]
   ["(on :gate PATTERN EFFECT)" "Gate an effect inside :fx; PATTERN accepts p, s, g, gate-seq, euclid, then, and times." "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(on :gate (g [0 1]) (delay :mix 0.4))])\n(start!)"]
   ["(post-fx [...])" "Render/master effect chain over an audio source." "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [(reverb :mix 0.2)])\n(start!)"]
   ["all-null effect" "An effect form whose provided controls are all null is skipped." ":fx [(filter :type null :cutoff null :res null)]"]
   [":mix" "Common effect wet mix from 0 to 1." ":mix 0.35"]
   [":feedback" "Common effect feedback from 0 to 0.95." ":feedback 0.32"]
   [":res" "Filter resonance from 0 to 1." ":res 0.45"]
   ["distortion drive" ":drive uses 0 to 10." ":fx [(distort :drive 2.5)]"]
   ["bitcrush ranges" ":bits 2 to 16; :rate 1 to 128." ":fx [(bitcrush :bits 8 :rate 4)]"]
   ["post-fx normalized" "granular density/spray/pitch-spread, spectral-freeze freeze-pos/sustain, and autopan depth use 0 to 1." "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [(granular :density 0.6)])\n(start!)"]
   ["tape-stop duration" ":duration uses 0.1 to 1." "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [(tape-stop :duration 0.5)])\n(start!)"]
   ["creative normalized" "lofi, vinyl, sidechain, radio, telephone, underwater, and crystal normalized controls use 0 to 1." ":fx [(lofi :amount 0.45)]"]
   ["modulation counts" "chorus voices 1-8, ensemble voices 2-12, phaser stages 1-12, dimension modes 1-4." ":fx [(chorus :voices 3)]"]
   ["modulation ranges" "tremolo, chorus, ensemble, CE-1, RE-301, flanger, phaser, small-stone, vibrato, and ring-mod reject out-of-range rate/depth controls." ":fx [(flanger :rate 0.25 :depth 0.002)]"]
   ["analog normalized" "tube, tape, studer-tape, and exciter normalized controls use 0 to 1; studer-tape speed uses 0 to 2." ":fx [(tape :saturation 0.35)]"]
   ["dynamics ranges" "fairchild input-gain, la2a peak-reduction, and 1176 input/attack/release use 0 to 1; fairchild time-constant uses 1 to 6; transient gain uses 0 to 8 / 0 to 4." ":fx [(1176 :attack 0.25 :release 0.45)]"]
   ["hardware normalized" "moog drive, 303 env-mod/accent, and buchla strike use 0 to 1." ":fx [(tb-303 :env-mod 0.6 :accent 0.4)]"]
   ["amp eq normalized" "neve, marshall, vox, fender, and pultec knob controls use 0 to 1." ":fx [(marshall-amp :gain 0.45 :tone 0.55)]"]
   ["reverb delay ranges" "reverb/delay models reject out-of-range decay, damping, size, time, and modulation controls." ":fx [(space-echo :time 0.25 :spring-mix 0.2)]"]
   ["time glitch ranges" "octaver, stutter, glitch, fade, adsr, and doppler reject out-of-range time/pitch controls." ":fx [(glitch :density 0.2 :slice-ms 25)]"]
   ["creative body ranges" "wavefolder, resonator, maximizer, harmonic-enhance, body, warmth, spatial, and crystal reject out-of-range bounded controls." ":fx [(wavefolder :folds 3 :gain 2 :symmetry 1)]"]])

(def compiler-forms
  [["(def name value)" "Bind reusable code." "(def lead (d :lead :src :sine-synth))"]
   ["(tracks ...)" "Group track forms." "(tracks (d :a :src :sine-synth :gate 1) (d :b :src :square-synth :gate 1))"]
   ["(by-scene ...)" "Choose value by scene." "(by-scene :intro c3 :else c4)"]
   ["(+ ...)" "Add numbers or transpose notes." "(+ c3 7)"]
   ["(- ...)" "Subtract numbers." "(- 4 1)"]
   ["(* ...)" "Multiply numbers." "(* 2 4)"]
   ["(/ ...)" "Divide numbers." "(/ 8 2)"]
   ["(and ...)" "Boolean numeric and." "(and [1 0] [1 1])"]
   ["(or ...)" "Boolean numeric or." "(or [1 0] [0 1])"]
   ["(not ...)" "Boolean numeric not." "(not [1 0])"]
   ["(map op ...)" "Apply op across patterns." "(map transpose [c3 d3] 12)"]
   ["(range ...)" "Generate numbers or notes." "(range c3 c4 2)"]
   ["(repeat N X)" "Repeat a value or vector." "(repeat 2 [1 0])"]
   ["(take N X)" "Take first N items." "(take 3 [1 2 3 4])"]
   ["(reverse X)" "Reverse a vector or pattern." "(reverse [c3 e3 g3])"]
   ["(rotate N X)" "Rotate a vector." "(rotate 1 [1 0 0])"]
   ["(interleave ...)" "Interleave vectors." "(interleave [1 1] [0 0])"]
   ["(every-n N hit rest)" "Hit every N steps." "(every-n 4 1 0)"]
   ["(choose ...)" "Seeded random choices." "(choose :count 4 :seed 1 [c3 e3])"]
   ["(rand-range ...)" "Seeded random numbers." "(rand-range :count 4 :min 0 :max 1)"]
   ["(scale root kind count)" "Generate scale notes." ":note (p (scale c3 :minor 8))"]
   ["(chord root kind)" "Generate named chord tones." ":note (p (chord c3 :minor7))"]
   ["(chord root [intervals])" "Generate custom chord tones from semitone offsets." ":note (p (chord c3 [0 3 7 10]))"]
   ["(shape values [pos])" "Pick 1-based positions from a vector, chord, or scale." ":note (p (shape (chord c2 :minor7) [2 4]))"]
   ["(arpeggio root kind)" "Chord tones as a sequence." ":note (p (arpeggio c3 :minor7))"]
   ["(arpeggio root [intervals])" "Custom chord tones as a sequence." ":note (p (arpeggio c3 [0 3 7 10]))"]
   ["(transpose X N)" "Transpose note or value." "(transpose c3 12)"]])

(def scale-kinds
  [":major" ":minor" ":natural-minor" ":harmonic-minor" ":melodic-minor"
   ":pentatonic" ":major-pentatonic" ":minor-pentatonic" ":blues"
   ":minor-blues" ":major-blues" ":dorian" ":phrygian" ":lydian"
   ":mixolydian" ":locrian" ":chromatic" ":whole-tone" ":diminished"
   ":whole-half-diminished" ":half-whole-diminished" ":bebop-major"
   ":bebop-dominant"])

(def chord-kinds
  [":major" ":minor" ":dim" ":diminished" ":aug" ":augmented" ":sus2" ":sus4"
   ":power" ":7" ":dom7" ":m7" ":minor7" ":maj7" ":major7"])

(def post-effect-labels
  #{"reverse" "tape-stop" "granular" "granular-stretch" "spectral-freeze"
    "haas" "stereo-widen" "stereo-imager" "width-enhance" "freq-shift"
    "autopan" "ping-pong-delay"})

(defn effect-reference-example [{:keys [label form]}]
  (cond
    (str/starts-with? form ":fx") form
    (contains? post-effect-labels label) (str "(d :lead :src :sine-synth :note c3 :gate 1)\n(post-fx [" form "])\n(start!)")
    :else (str ":fx [" form "]")))

(defn keyword-contract? [contract]
  (and contract
       (str/includes? contract "type: keyword")
       (str/includes? contract "range:")))

(defn keyword-contract-summary [[param contract]]
  (let [choices (some-> contract
                        (str/split #"range:")
                        second
                        str/trim
                        (str/replace "|" ", "))]
    (when-not (str/blank? choices)
      (str param " " choices))))

(defn effect-keyword-summary [label]
  (let [summaries (->> (get catalog/effect-type-contracts label)
                       (filter (fn [[_ contract]] (keyword-contract? contract)))
                       (map keyword-contract-summary)
                       (remove str/blank?))]
    (when (seq summaries)
      (str " Keywords: " (str/join "; " summaries) "."))))

(defn effect-rows []
  (->> catalog/effect-options
       (remove #(= "FX Vector" (:label %)))
       (map (fn [{:keys [label form]}]
              [label
               (str "Apply " label "." (or (effect-keyword-summary label) ""))
               (effect-reference-example {:label label :form form})]))))

(defn scale-rows []
  (map #(vector % "Scale name." (str ":note (p (scale c3 " % " 8))")) scale-kinds))

(defn chord-rows []
  (map #(vector % "Chord name." (str ":note (p (chord c3 " % "))")) chord-kinds))

(defn doc-sections []
  [{:id "quick-start" :title "Quick Start" :rows quick-start}
   {:id "syntax-basics" :title "Syntax Basics" :rows syntax-basics}
   {:id "top-level-forms" :title "Top-Level Forms" :rows top-level-forms}
   {:id "compatibility-aliases" :title "Compatibility Aliases" :rows compatibility-aliases}
   {:id "scene-options" :title "Scene Options" :rows scene-options}
   {:id "track-parameters" :title "Track Parameters" :rows track-params}
   {:id "pattern-note-forms" :title "Pattern And Note Forms" :rows pattern-forms}
   {:id "compile-time-forms" :title "Compile-Time Forms" :rows compiler-forms}
   {:id "scale-kinds" :title "Scale Kinds" :rows (scale-rows)}
   {:id "chord-kinds" :title "Chord Kinds" :rows (chord-rows)}
   {:id "effect-syntax" :title "Effect Syntax" :rows effect-forms}
   {:id "effects" :title "Effects" :rows (effect-rows)}])

(defn language-reference-html
  ([]
   (language-reference-html #{}))
  ([collapsed-section-ids]
   (let [sections (doc-sections)]
    (str "<html><head><style>"
         "body{font-family:monospace;font-size:12px;margin:14px;color:#202020;}"
         "h1{font-size:22px;margin:0 0 4px 0;}h2{font-size:16px;margin:22px 0 8px 0;}"
         "p{margin:3px 0 5px 0;}ul{margin-top:6px;}li{margin:2px 0;}"
         "a{color:#2457a6;text-decoration:none;}code{font-weight:bold;color:#111;}"
         ".entry{margin:0 0 12px 0;}pre{margin:3px 0 0 16px;color:#4b4b4b;}"
         ".index a{display:inline-block;margin:0 10px 7px 0;}"
         "</style></head><body>"
         "<a name='top'></a>"
         "<h1>MeScript Language Reference</h1>"
         (toc-html sections)
         (apply str (map #(section-html collapsed-section-ids %) sections))
         "</body></html>"))))

(defn language-reference-text []
  (language-reference-html))

(defn all-section-ids []
  (set (map :id (doc-sections))))

(defn render-language-reference! [^JTextPane text collapsed-section-ids]
  (.setText text (language-reference-html collapsed-section-ids))
  (.setCaretPosition text 0))

(defn open-reference-section! [^JTextPane text collapsed-section-ids section-id]
  (when (contains? @collapsed-section-ids section-id)
    (swap! collapsed-section-ids disj section-id)
    (render-language-reference! text @collapsed-section-ids))
  (.scrollToReference text section-id))

(defn search-document [^JTextPane text query]
  (let [doc (.getDocument text)
        body (.getText doc 0 (.getLength doc))
        needle (str/lower-case query)
        haystack (str/lower-case body)
        start (min (count haystack) (inc (max 0 (.getCaretPosition text))))
        forward (.indexOf haystack needle start)
        wrapped (when (neg? forward)
                  (.indexOf haystack needle 0))]
    (when-not (neg? (or wrapped forward))
      (let [idx (if (neg? forward) wrapped forward)]
        (.requestFocusInWindow text)
        (.setCaretPosition text idx)
        (.moveCaretPosition text (+ idx (count query)))
        idx))))

(defn section-contains-query? [{:keys [title rows]} query]
  (let [needle (str/lower-case query)
        haystack (str/lower-case
                   (str title "\n"
                        (str/join "\n"
                                  (map (fn [[name description example]]
                                         (str name "\n" description "\n" example))
                                       rows))))]
    (not (neg? (.indexOf haystack needle)))))

(defn expand-search-matches! [^JTextPane text collapsed-section-ids query]
  (let [matching-section-ids (->> (doc-sections)
                                  (filter #(section-contains-query? % query))
                                  (map :id)
                                  set)
        still-collapsed (apply disj @collapsed-section-ids matching-section-ids)]
    (when (not= still-collapsed @collapsed-section-ids)
      (reset! collapsed-section-ids still-collapsed)
      (render-language-reference! text @collapsed-section-ids))))

(defn search-reference! [^JTextPane text ^JTextField field collapsed-section-ids]
  (let [query (str/trim (.getText field))]
    (when-not (str/blank? query)
      (expand-search-matches! text collapsed-section-ids query)
      (search-document text query))))

(defn reference-bottom-menu [^JTextPane text collapsed-section-ids]
  (let [panel (JPanel. (BorderLayout. 6 0))
        field (JTextField.)
        buttons (JPanel.)
        search-button (JButton. "Search")
        collapse-button (JButton. "Collapse All")
        top-button (JButton. "Back to Top")
        search! #(search-reference! text field collapsed-section-ids)]
    (.setName panel "mescript-language-reference-bottom-menu")
    (.setName field "mescript-language-reference-search")
    (.setName search-button "mescript-language-reference-search-button")
    (.setName collapse-button "mescript-language-reference-collapse-button")
    (.setName top-button "mescript-language-reference-top-button")
    (.addActionListener field
                        (reify ActionListener
                          (actionPerformed [_ _] (search!))))
    (.addActionListener search-button
                        (reify ActionListener
                          (actionPerformed [_ _] (search!))))
    (.addActionListener collapse-button
                        (reify ActionListener
                          (actionPerformed [_ _]
                            (reset! collapsed-section-ids (all-section-ids))
                            (render-language-reference! text @collapsed-section-ids)
                            (.scrollToReference text "top")
                            (.setCaretPosition text 0))))
    (.addActionListener top-button
                        (reify ActionListener
                          (actionPerformed [_ _]
                            (.scrollToReference text "top")
                            (.setCaretPosition text 0))))
    (.add buttons search-button)
    (.add buttons collapse-button)
    (.add buttons top-button)
    (.add panel field BorderLayout/CENTER)
    (.add panel buttons BorderLayout/EAST)
    panel))

(defn show-language-reference! [^JFrame parent]
  (let [collapsed-section-ids (atom (all-section-ids))
        frame (JFrame. "MeScript Language Reference")
        text (doto (JTextPane.)
               (.setEditable false)
               (.setContentType "text/html")
               (.setText (language-reference-html @collapsed-section-ids))
               (.setCaretPosition 0)
               (.setFont (Font. Font/MONOSPACED Font/PLAIN 12)))
        scroll (JScrollPane. text)
        bottom-menu (reference-bottom-menu text collapsed-section-ids)]
    (.addHyperlinkListener
      text
      (reify HyperlinkListener
        (hyperlinkUpdate [_ event]
          (when (= HyperlinkEvent$EventType/ACTIVATED (.getEventType event))
            (let [description (.getDescription event)]
              (cond
                (and description (str/starts-with? description "#toggle:"))
                (let [section-id (subs description (count "#toggle:"))]
                  (swap! collapsed-section-ids
                         (fn [ids]
                           (if (contains? ids section-id)
                             (disj ids section-id)
                             (conj ids section-id))))
                  (render-language-reference! text @collapsed-section-ids)
                  (.scrollToReference text section-id))

                (and description (str/starts-with? description "#"))
                (let [section-id (subs description 1)]
                  (if (contains? (all-section-ids) section-id)
                    (open-reference-section! text collapsed-section-ids section-id)
                    (.scrollToReference text section-id)))))))))
    (.setPreferredSize scroll (Dimension. 760 620))
    (.add (.getContentPane frame) scroll BorderLayout/CENTER)
    (.add (.getContentPane frame) bottom-menu BorderLayout/SOUTH)
    (.setName frame "mescript-language-reference-frame")
    (.setName text "mescript-language-reference-text")
    (.pack frame)
    (.setLocationRelativeTo frame parent)
    (.setVisible frame true)
    frame))
