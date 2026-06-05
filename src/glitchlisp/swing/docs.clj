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
  [["(bpm N)" "Set tempo." "(bpm 120)"]
   ["(d :id ...)" "Define a playable pattern." "(d :lead :src :sine-synth :note c3 :gate 1)"]
   ["(scene :name ...)" "Define a scene." "(scene :intro :repeat 1 lead)"]
   ["(block :name ...)" "Alias for scene." "(block :intro :repeat 1 lead)"]
   ["(play-scene :name)" "Start a scene." "(play-scene :intro)"]
   ["(play-block :name)" "Alias for play-scene." "(play-block :intro)"]
   ["(cue :name)" "Alias for play-scene." "(cue :intro)"]
   ["(start!)" "Start top-level tracks." "(start!)"]
   ["(stop!)" "Stop playback." "(stop!)"]
   ["(play-note NOTE)" "Play one sine note." "(play-note c3)"]
   ["(post-fx [...])" "Apply render/master effects." "(post-fx [(reverb :mix 0.2)])"]
   ["(master-fx [...])" "Alias for post-fx." "(master-fx [(tape :saturation 0.4)])"]
   ["(mute :id)" "Mute a track." "(mute :lead)"]
   ["(unmute :id)" "Unmute a track." "(unmute :lead)"]
   ["(solo :id)" "Solo a track." "(solo :lead)"]
   ["(unsolo :id)" "Unsolo a track." "(unsolo :lead)"]
   ["(clear :id)" "Remove a track." "(clear :lead)"]
   ["(clear-all)" "Clear runtime state." "(clear-all)"]])

(def syntax-basics
  [["; comment" "Comment to end of line." "; drums"]
   [":keyword" "Name used as an id or option." ":intro"]
   ["[...]" "Vector of values." "[1 0 1 0]"]
   ["(...)" "Form call." "(bpm 120)"]
   ["NOTE" "Pitch name; sharps and flats are supported." "c3 eb3 f#4"]
   ["NUMBER" "Numeric value." "0.25"]
   ["STRING" "Text value." "\"kick.wav\""]
   ["null" "Leave default value." ":amp null"]
   ["nil" "Alias for null." ":dur nil"]])

(def scene-options
  [[":repeat N" "Repeat count; 0 loops forever." "(scene :a :repeat 2 ...)"]
   [":repeats N" "Alias for repeat." "(scene :a :repeats 2 ...)"]
   [":times N" "Alias for repeat." "(scene :a :times 2 ...)"]
   [":steps N" "Set scene length in steps." "(scene :a :steps 16 ...)"]
   [":length N" "Alias for steps." "(scene :a :length 16 ...)"]
   [":bars N" "Set length in 16-step bars." "(scene :a :bars 2 ...)"]
   [":steps-of :id" "Use a track's cycle length." "(scene :a :steps-of :kick ...)"]
   [":length-of :id" "Alias for steps-of." "(scene :a :length-of :kick ...)"]
   [":next :scene" "Move to another scene." "(scene :a :repeat 1 :next :b ...)"]])

(def track-params
  [[":src" "Oscillator or sample source." ":src :additive"]
   [":note" "Note, chord, or note pattern." ":note (p [c3 e3 g3])"]
   [":gate" "Hit/rest pattern." ":gate (p [1 0 1 0])"]
   [":dur" "Voice duration in seconds." ":dur 0.2"]
   [":amp" "Track amplitude." ":amp 0.4"]
   [":fx" "Track effect chain." ":fx [(delay :time 0.125)]"]
   [":every" "Play every N transport steps." ":every 2"]
   [":offset" "Offset track step index." ":offset 1"]
   [":detune-cents" "Detune in cents." ":detune-cents 7"]
   [":detune" "Alias for detune-cents." ":detune 7"]
   [":phase" "Oscillator phase." ":phase 0.25"]
   [":pulse-width" "Pulse width." ":pulse-width 0.4"]
   [":pw" "Alias for pulse-width." ":pw 0.4"]
   [":morph" "Morph position." ":morph 0.5"]
   [":gain" "Oscillator gain." ":gain 1.2"]
   [":unison" "Number of unison voices." ":unison 4"]
   [":unison-detune" "Unison detune cents." ":unison-detune 8"]
   [":unison-spread" "Stereo unison width." ":unison-spread 0.7"]
   [":fm-ratio" "FM/sync ratio." ":fm-ratio 2"]
   [":fm-depth" "FM depth." ":fm-depth 3"]
   [":harmonics" "Additive harmonic levels." ":harmonics [1 0.5 0.25]"]
   [":sample-path" "Load a wav file." ":sample-path \"kick.wav\""]
   [":sample" "Alias for sample-path." ":sample \"kick.wav\""]
   [":sample-data" "Inline sample values." ":sample-data [0 1 0 -1]"]])

(def pattern-forms
  [["(p [...])" "Step pattern." "(p [1 0 1 0])"]
   ["(p :repeat N [...])" "Repeat a pattern." "(p :repeat 2 [1 0])"]
   ["(then A B ...)" "Play gate stages in order; the final stage loops." "(p (then (times 2 [0 0 0 1]) [1 0 1 0]))"]
   ["(times N PATTERN)" "Repeat a gate pattern N times for a finite stage." "(times 2 [1 0 0 0])"]
   ["(p [[...]])" "Nested note values play together as a chord step." "(p [[c3 eb3 g3]])"]
   ["(s [...])" "Advance notes on hits." "(s [c3 e3 g3])"]
   ["(gs [...])" "Advance notes on gate slots." "(gs [c3 e3 g3])"]
   ["(gate-seq [...])" "Alias for gs." "(gate-seq [c3 e3])"]
   ["(gate_seq [...])" "Alias for gs." "(gate_seq [c3 e3])"]
   ["(euclid P S)" "Euclidean gate pattern." "(euclid 5 16)"]
   ["(euclid-rot P S R)" "Rotated Euclidean pattern." "(euclid-rot 5 16 2)"]
   ["(gate-hold N)" "Extend a hit by N slots." "(p [1 (gate-hold 2) 1])"]
   ["1_N" "Short gate hold; later hits in the held span can still play." "(p [1 1_2 1 1])"]
   ["Nested gates" "Subdivide a step." "(p [[1 1] 0 1])"]])

(def effect-forms
  [[":fx [...]" "Track effect chain." ":fx [(delay :time 0.125)]"]
   ["(on :gate PATTERN EFFECT)" "Gate an effect." "(on :gate (p [0 1]) (delay :mix 0.4))"]
   ["(post-fx [...])" "Render/master effect chain." "(post-fx [(reverb :mix 0.2)])"]])

(def compiler-forms
  [["(def name value)" "Bind reusable code." "(def lead (d :lead :src :sine-synth))"]
   ["(tracks ...)" "Group track forms." "(tracks (d :a ...) (d :b ...))"]
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
   ["(rev X)" "Runtime alias for reverse." "(rev (p [1 0]))"]
   ["(rotate N X)" "Rotate a vector." "(rotate 1 [1 0 0])"]
   ["(interleave ...)" "Interleave vectors." "(interleave [1 1] [0 0])"]
   ["(every-n N hit rest)" "Hit every N steps." "(every-n 4 1 0)"]
   ["(choose ...)" "Seeded random choices." "(choose :count 4 :seed 1 [c3 e3])"]
   ["(rand-range ...)" "Seeded random numbers." "(rand-range :count 4 :min 0 :max 1)"]
   ["(scale root kind count)" "Generate scale notes." "(scale c3 :minor 8)"]
   ["(chord root kind)" "Generate named chord tones." "(chord c3 :minor7)"]
   ["(chord root [intervals])" "Generate custom chord tones from semitone offsets." "(chord c3 [0 3 7 10])"]
   ["(shape values [pos])" "Pick 1-based positions from a vector, chord, or scale." "(shape (chord c2 :minor7) [2 4])"]
   ["(arpeggio root kind)" "Chord tones as a sequence." "(arpeggio c3 :minor7)"]
   ["(arpeggio root [intervals])" "Custom chord tones as a sequence." "(arpeggio c3 [0 3 7 10])"]
   ["(arp root kind)" "Alias for arpeggio." "(arp c3 :minor7)"]
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

(defn effect-rows []
  (->> catalog/effect-options
       (remove #(= "FX Vector" (:label %)))
       (map (fn [{:keys [label form]}]
              [label (str "Apply " label ".") form]))))

(defn scale-rows []
  (map #(vector % "Scale name." (str "(scale c3 " % " 8)")) scale-kinds))

(defn chord-rows []
  (map #(vector % "Chord name." (str "(chord c3 " % ")")) chord-kinds))

(defn doc-sections []
  [{:id "syntax-basics" :title "Syntax Basics" :rows syntax-basics}
   {:id "top-level-forms" :title "Top-Level Forms" :rows top-level-forms}
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
