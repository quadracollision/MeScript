(ns glitchlisp.swing.editor
  (:require [clojure.string :as str]
            [glitchlisp.swing.shared :as shared])
  (:import
    [java.awt Color Dimension Font Rectangle]
    [java.awt.event ActionListener MouseAdapter MouseEvent]
    [javax.swing.event CaretListener DocumentListener UndoableEditListener]
    [javax.swing.text DefaultCaret DefaultHighlighter$DefaultHighlightPainter JTextComponent SimpleAttributeSet StyleConstants StyledDocument]
    [javax.swing AbstractAction BorderFactory JFrame JLabel JComponent JMenuItem JOptionPane JPopupMenu JTextField JTextPane JViewport KeyStroke SwingUtilities Timer]
    [javax.swing.undo UndoManager]))

(def set-status! shared/set-status!)

(defn document-length [^JTextComponent editor]
  (.getLength (.getDocument editor)))

(defn safe-caret-position [^JTextComponent editor]
  (let [length (document-length editor)]
    (try
      (-> (.getCaretPosition editor)
          (max 0)
          (min length))
      (catch Exception _
        length))))

(defn ensure-valid-caret! [^JTextComponent editor]
  (let [position (safe-caret-position editor)]
    (try
      (.setCaretPosition editor position)
      (catch Exception _
        (.setCaretPosition editor (document-length editor))))
    (safe-caret-position editor)))

(defn safe-selection-range [^JTextComponent editor]
  (let [length (document-length editor)
        start (-> (.getSelectionStart editor) (max 0) (min length))
        end (-> (.getSelectionEnd editor) (max 0) (min length))]
    [(min start end) (max start end)]))

(defn safe-editor-caret []
  (proxy [DefaultCaret] []
    (focusLost [event]
      (try
        (when-let [component (.getComponent this)]
          (let [length (.getLength (.getDocument component))
                dot (-> (.getDot this) (max 0) (min length))]
            (.setDot this dot)))
        (proxy-super focusLost event)
        (catch Exception _)))))

(defn insert-at-caret! [^JTextComponent editor text]
  (ensure-valid-caret! editor)
  (let [[start end] (safe-selection-range editor)
        doc (.getDocument editor)]
    (.remove doc start (- end start))
    (.insertString doc start text nil)
    (.setCaretPosition editor (+ start (count text)))
    (.requestFocusInWindow editor)))

(defn replace-selection-with-text! [^JTextComponent editor text]
  (insert-at-caret! editor text))

(defn insert-newline-safely! [^JTextComponent editor]
  (let [doc (.getDocument editor)
        length (.getLength doc)
        position (-> (try
                       (.getCaretPosition editor)
                       (catch Exception _ length))
                     (max 0)
                     (min length))]
    (.insertString doc position "\n" nil)
    (try
      (.setCaretPosition editor (inc position))
      (catch Exception _))))

(defn replace-text-range! [^JTextComponent editor text start end]
  (let [doc (.getDocument editor)
        length (.getLength doc)
        start (max 0 (min start length))
        end (max start (min end length))]
    (.remove doc start (- end start))
    (.insertString doc start text nil)))

(defn insert-text-at! [^JTextComponent editor text offset]
  (.insertString (.getDocument editor) offset text nil))

(defn leading-spaces [text]
  (apply str (take-while #(= % \space) text)))

(defn bounded-subs
  ([text start]
   (bounded-subs text start (count text)))
  ([text start end]
   (let [length (count text)
         start (max 0 (min start length))
         end (max start (min end length))]
     (subs text start end))))

(defn current-line-before-caret [^JTextComponent editor]
  (let [text (.getText editor)
        caret (max 0 (min (.getCaretPosition editor) (count text)))
        search-from (dec caret)
        line-start (if (neg? search-from)
                     0
                     (let [found (.lastIndexOf text "\n" search-from)]
                       (if (neg? found) 0 (inc found))))]
    (bounded-subs text line-start caret)))

(defn delimiter-balance [line]
  (loop [chars (seq line)
         balance 0
         in-string? false
         escape? false]
    (if-let [ch (first chars)]
      (cond
        escape? (recur (next chars) balance in-string? false)
        (= ch \\) (recur (next chars) balance in-string? true)
        (= ch \") (recur (next chars) balance (not in-string?) false)
        in-string? (recur (next chars) balance in-string? false)
        (#{\( \[} ch) (recur (next chars) (inc balance) false false)
        (#{\) \]} ch) (recur (next chars) (dec balance) false false)
        :else (recur (next chars) balance false false))
      balance)))

(def opening-delimiters
  {\( \)
   \[ \]})

(def closing-delimiters
  {\) \(
   \] \[})

(defn line-start-offset [text offset]
  (let [bounded (max 0 (min offset (count text)))
        search-from (dec bounded)]
    (if (neg? search-from)
      0
      (let [found (.lastIndexOf text "\n" search-from)]
        (if (neg? found) 0 (inc found))))))

(defn line-indent-at-offset [text offset]
  (let [bounded (max 0 (min offset (count text)))
        start (line-start-offset text bounded)
        line (bounded-subs text start bounded)]
    (leading-spaces line)))

(defn line-before-offset [text offset]
  (let [bounded (max 0 (min offset (count text)))
        start (line-start-offset text bounded)]
    (bounded-subs text start bounded)))

(defn safe-leading-indent-before [text caret]
  (try
    (leading-spaces (line-before-offset (or text "") caret))
    (catch Throwable _
      "")))

(defn blank-line-before-caret? [text caret]
  (try
    (clojure.string/blank? (line-before-offset (or text "") caret))
    (catch Throwable _
      true)))

(defn delimiter-stack [text]
  (loop [idx 0
         stack []
         in-string? false
         escape? false
         in-comment? false]
    (if (< idx (count text))
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) stack in-string? false (not= ch \newline))

          escape?
          (recur (inc idx) stack in-string? false false)

          (and in-string? (= ch \\))
          (recur (inc idx) stack true true false)

          (= ch \")
          (recur (inc idx) stack (not in-string?) false false)

          in-string?
          (recur (inc idx) stack true false false)

          (= ch \;)
          (recur (inc idx) stack false false true)

          (contains? opening-delimiters ch)
          (recur (inc idx) (conj stack {:ch ch :offset idx}) false false false)

          (contains? closing-delimiters ch)
          (recur (inc idx) (if (seq stack) (pop stack) stack) false false false)

          :else
          (recur (inc idx) stack false false false)))
      stack)))

(defn form-head-near-open [text open-offset]
  (let [open-offset (max 0 (min open-offset (dec (count text))))]
    (when (and (seq text) (= (.charAt text open-offset) \())
      (let [after-open (bounded-subs text (inc open-offset))
          trimmed (clojure.string/triml after-open)]
        (some-> (re-find #"^([^\s\)\]]+)" trimmed)
                second)))))

(defn first-token-column-after [text offset]
  (loop [idx (inc offset)]
    (cond
      (>= idx (count text)) nil
      (= (.charAt text idx) \newline) nil
      (Character/isWhitespace (.charAt text idx)) (recur (inc idx))
      :else (- idx (line-start-offset text idx)))))

(defn vector-content-indent [text vector-open]
  (apply str
         (repeat (or (first-token-column-after text vector-open)
                     (inc (- vector-open (line-start-offset text vector-open))))
                 " ")))

(defn open-vector-before-caret [text caret]
  (let [caret (max 0 (min caret (count text)))
        stack (delimiter-stack (bounded-subs text 0 caret))]
    (some (fn [{:keys [ch offset]}]
            (when (= ch \[)
              offset))
          (reverse stack))))

(defn vector-enter-indent [text caret line-start]
  (when-let [vector-open (open-vector-before-caret text caret)]
    (when (>= vector-open line-start)
      (vector-content-indent text vector-open))))

(defn smart-next-line-indent [text caret]
  (let [text (or text "")
        fallback safe-leading-indent-before]
    (try
      (let [caret (max 0 (min caret (count text)))
            before (bounded-subs text 0 caret)
            stack (delimiter-stack before)
            line-start (line-start-offset text caret)
            current-line (line-before-offset text caret)
            fallback-indent (fallback text caret)]
        (if (or (zero? caret)
                (clojure.string/blank? current-line))
          ""
          (or (vector-enter-indent text caret line-start)
              (if-let [{:keys [ch offset]} (peek stack)]
                (let [base (line-indent-at-offset text offset)
                      head (form-head-near-open text offset)]
                  (cond
                    (= ch \[) (vector-content-indent text offset)
                    (contains? #{"d" "sample"} head) (str base "   ")
                    (contains? #{"scene" "block" "def" "tracks"} head) (str base "  ")
                    :else (str base "  ")))
                fallback-indent))))
      (catch Throwable _
        (fallback text caret)))))

(defn line-end-offset [text offset]
  (let [idx (.indexOf text "\n" (max 0 (min offset (count text))))]
    (if (neg? idx) (count text) idx)))

(declare matching-close enclosing-track-range code-visible-text)

(defn previous-track-open [text caret]
  (let [visible (code-visible-text text)
        d-idx (.lastIndexOf visible "(d " caret)
        sample-idx (.lastIndexOf visible "(sample " caret)]
    (max d-idx sample-idx)))

(defn previous-track-indent [text track-start]
  (loop [idx (previous-track-open text (max 0 (dec track-start)))]
    (when (>= idx 0)
      (let [previous-idx (previous-track-open text (dec idx))]
        (if-let [end (matching-close text idx \( \))]
          (if (< end track-start)
            (leading-spaces (subs text (line-start-offset text idx) idx))
            (recur previous-idx))
          (recur previous-idx))))))

(defn replace-line-indent [line indent]
  (str indent (clojure.string/replace-first line #"^[ \t]*" "")))

(defn reindent-block-text [block target-indent]
  (let [lines (clojure.string/split block #"\n" -1)
        old-indent (leading-spaces (first lines))]
    (->> lines
         (map (fn [line]
                (cond
                  (clojure.string/blank? line) ""
                  (clojure.string/starts-with? line old-indent)
                  (str target-indent (subs line (count old-indent)))
                  :else
                  (replace-line-indent line target-indent))))
         (clojure.string/join "\n"))))

(defn align-current-line-to-vector [text caret]
  (let [caret (max 0 (min caret (count text)))
        line-start (line-start-offset text caret)
        vector-open (open-vector-before-caret text caret)]
    (when vector-open
      (let [line-end (line-end-offset text caret)
            line (subs text line-start line-end)
            replacement (replace-line-indent line (vector-content-indent text vector-open))]
        {:start line-start
         :end line-end
         :text replacement}))))

(defn align-current-track-to-previous [text caret]
  (when-let [[start end] (enclosing-track-range text caret)]
    (when-let [target-indent (previous-track-indent text start)]
      (let [block-start (line-start-offset text start)
            block-end (line-end-offset text (inc end))
            block (subs text block-start block-end)
            replacement (reindent-block-text block target-indent)]
        {:start block-start
         :end block-end
         :text replacement}))))

(defn install-auto-indent! [^JTextComponent editor]
  (let [enter (KeyStroke/getKeyStroke "ENTER")
        tab (KeyStroke/getKeyStroke "TAB")
        action-key "glitchlisp-auto-indent"
        tab-action-key "glitchlisp-align-track-indent"]
    (.put (.getInputMap editor JComponent/WHEN_FOCUSED) enter action-key)
    (.put (.getActionMap editor)
          action-key
          (proxy [AbstractAction] []
            (actionPerformed [_]
              (try
                (let [caret (ensure-valid-caret! editor)
                      text (.getText editor)
                      indent (try
                               (smart-next-line-indent text caret)
                               (catch Throwable _ ""))]
                  (try
                    (replace-selection-with-text! editor (str "\n" indent))
                    (catch Throwable _
                      (insert-newline-safely! editor))))
                (catch Throwable _
                  (insert-newline-safely! editor))))))
    (.put (.getInputMap editor JComponent/WHEN_FOCUSED) tab tab-action-key)
    (.put (.getActionMap editor)
          tab-action-key
          (proxy [AbstractAction] []
            (actionPerformed [_]
              (let [source (.getText editor)
                    caret (.getCaretPosition editor)
                    edit (or (align-current-line-to-vector source caret)
                             (align-current-track-to-previous source caret))]
                (if-let [{:keys [start end text]} edit]
                  (do
                    (replace-text-range! editor text start end)
                    (.setCaretPosition editor (+ start (count text))))
                  (.replaceSelection editor "  "))))))))

(defn line-column-for-offset [text offset]
  (let [bounded (max 0 (min offset (count text)))
        before (subs text 0 bounded)
        line (inc (count (filter #(= % \newline) before)))
        line-start (inc (.lastIndexOf before "\n"))]
    {:line line
     :column (inc (- bounded line-start))}))

(defn offset-for-line-column [text line column]
  (let [line (max 1 (int line))
        column (max 1 (int column))]
    (loop [idx 0
           current-line 1]
      (cond
        (= current-line line)
        (min (count text) (+ idx (dec column)))

        (>= idx (count text))
        (count text)

        :else
        (let [next-newline (.indexOf text "\n" idx)]
          (if (neg? next-newline)
            (count text)
            (recur (inc next-newline) (inc current-line))))))))

(defn line-column-from-message [message]
  (when-let [[_ line column] (and message
                                  (re-find #"(?i)line\s+([0-9]+),?\s+(?:column|col)\s+([0-9]+)"
                                           message))]
    {:line (Integer/parseInt line)
     :column (Integer/parseInt column)}))

(defn clean-error-message [ex]
  (let [message (or (.getMessage ex) (str ex))
        data (ex-data ex)
        data-location (select-keys data [:line :column])
        location (or (when (and (:line data-location) (:column data-location))
                       data-location)
                     (line-column-from-message message))
        detail (-> message
                   (clojure.string/replace #"(?i)\s+at\s+line\s+[0-9]+,?\s+(?:column|col)\s+[0-9]+" "")
                   (clojure.string/replace #"(?i)^error:\s*" "")
                   (clojure.string/replace #"(?i)^render failed:\s*" "")
                   (clojure.string/replace #"(?i)^(?:mismatched|missing)\s+delimiter:\s*" "")
                   clojure.string/trim)]
    (if (and (:line location) (:column location))
      (str "line " (:line location) ", col " (:column location) ": " detail)
      detail)))

(defn syntax-exception [source offset message]
  (let [{:keys [line column]} (line-column-for-offset source offset)]
    (ex-info (str "line " line ", col " column ": " message)
             {:offset offset
              :line line
              :column column})))

(defn validate-delimiters! [source]
  (loop [idx 0
         stack []
         in-string? false
         escape? false
         in-comment? false]
    (if (< idx (count source))
      (let [ch (.charAt source idx)]
        (let [[next-idx next-stack next-in-string? next-escape? next-in-comment?]
              (cond
                in-comment?
                [(inc idx) stack in-string? false (not= ch \newline)]

                escape?
                [(inc idx) stack in-string? false false]

                (and in-string? (= ch \\))
                [(inc idx) stack true true false]

                (= ch \")
                [(inc idx) stack (not in-string?) false false]

                in-string?
                [(inc idx) stack true false false]

                (= ch \;)
                [(inc idx) stack false false true]

                (contains? opening-delimiters ch)
                [(inc idx) (conj stack {:ch ch :offset idx}) false false false]

                (contains? closing-delimiters ch)
                (let [expected-open (closing-delimiters ch)
                      top (peek stack)]
                  (cond
                    (nil? top)
                    (throw (syntax-exception source idx (str "Unmatched delimiter: " ch)))

                    (= (:ch top) expected-open)
                    [(inc idx) (pop stack) false false false]

                   :else
                   (let [expected-close (opening-delimiters (:ch top))]
                     (throw (syntax-exception source idx
                                               (str "expected "
                                                    expected-close " before " ch))))))

                :else
                [(inc idx) stack false false false])]
          (recur next-idx next-stack next-in-string? next-escape? next-in-comment?)))
      (cond
        in-string?
        (throw (syntax-exception source (max 0 (dec (count source))) "Unterminated string"))

        (seq stack)
        (let [{:keys [ch offset]} (peek stack)
              expected-close (opening-delimiters ch)]
          (throw (syntax-exception source offset
                                   (str "expected "
                                        expected-close " to close " ch))))

        :else
        true))))

(def error-highlight-painter
  (DefaultHighlighter$DefaultHighlightPainter. (Color. 255 210 210)))

(def paren-match-painter
  (DefaultHighlighter$DefaultHighlightPainter. (Color. 190 225 255)))

(def paren-peer-painter
  (DefaultHighlighter$DefaultHighlightPainter. (Color. 205 245 205)))

(def paren-unmatched-painter
  (DefaultHighlighter$DefaultHighlightPainter. (Color. 255 190 190)))

(def error-highlight-key "glitchlisp.errorHighlight")
(def paren-highlight-key "glitchlisp.parenHighlights")
(def paren-refresh-timer-key "glitchlisp.parenRefreshTimer")
(def live-step-highlight-key "glitchlisp.liveStepRange")
(def live-step-highlight-rects-key "glitchlisp.liveStepRects")
(def live-scene-highlight-key "glitchlisp.liveSceneRange")
(def live-scene-highlight-segments-key "glitchlisp.liveSceneSegments")
(def live-scene-segments-cache-key "glitchlisp.liveSceneSegmentsCache")
(def live-step-repaint-key "glitchlisp.liveStepRepaintRange")
(def live-gate-ranges-key "glitchlisp.liveGateRanges")
(def live-gate-ranges-text-key "glitchlisp.liveGateRangesText")
(def live-focus-caret-key "glitchlisp.liveFocusCaret")
(def live-focus-range-key "glitchlisp.liveFocusRange")
(def live-scene-ranges-key "glitchlisp.liveSceneRanges")
(def live-scene-contexts-key "glitchlisp.liveSceneContexts")
(def live-active-gate-entries-key "glitchlisp.liveActiveGateEntries")
(def live-focused-active-gate-entries-key "glitchlisp.liveFocusedActiveGateEntries")
(def live-resolved-step-ranges-key "glitchlisp.liveResolvedStepRanges")
(def live-resolved-step-ranges-limit 256)
(def live-repaint-pump-key "glitchlisp.liveRepaintPump")
(def live-repaint-pump-delay-ms 16)
(def syntax-refreshing-key "glitchlisp.syntaxRefreshing")
(def editor-undo-manager-key "glitchlisp.editorUndoManager")
(def editor-edit-controls-key "glitchlisp.editControlsInstalled")
(def editor-context-menu-key "glitchlisp.contextMenu")
(def live-highlight-delay-ms 0)
(def syntax-refresh-delay-ms 45)
(def paren-refresh-delay-ms 35)
(def syntax-max-highlight-chars 60000)
(def live-cursor-profile-property "glitchlisp.liveCursorProfile")
(def live-cursor-profile-env "GLITCHLISP_LIVE_CURSOR_PROFILE")
(def live-cursor-profile-report-every 120)
(defonce live-cursor-profile-state
  (atom {:samples 0
         :stats {}}))

(defn force-editor-repaint! [^JTextComponent editor]
  (.revalidate editor)
  (.repaint editor))

(defn live-cursor-profile-enabled? []
  (let [property (System/getProperty live-cursor-profile-property)
        env (System/getenv live-cursor-profile-env)]
    (or (= "true" (str/lower-case (str property)))
        (= "1" env)
        (= "true" (str/lower-case (str env))))))

(defn nanos->ms [nanos]
  (/ (double nanos) 1000000.0))

(defn update-live-cursor-stat [stats k nanos]
  (update stats k
          (fn [stat]
            (let [stat (or stat {:n 0 :sum 0 :max 0})]
              {:n (inc (:n stat))
               :sum (+ (:sum stat) nanos)
               :max (max (:max stat) nanos)}))))

(defn live-cursor-stat-summary [label stat]
  (when (pos? (:n stat 0))
    (format "%s avg=%.2fms max=%.2fms"
            label
            (nanos->ms (/ (:sum stat) (:n stat)))
            (nanos->ms (:max stat)))))

(defn print-live-cursor-profile! [state]
  (let [stats (:stats state)
        parts (keep identity
                    [(live-cursor-stat-summary "receive->EDT" (:receive-to-edt stats))
                     (live-cursor-stat-summary "queue" (:queue stats))
                     (live-cursor-stat-summary "highlight" (:highlight stats))
                     (live-cursor-stat-summary "paint" (:paint stats))])]
    (when (seq parts)
      (println (str "[live cursor profile] samples=" (:samples state)
                    " " (str/join " | " parts))))))

(defn record-live-cursor-timing! [k nanos]
  (when (and (live-cursor-profile-enabled?) (not (neg? nanos)))
    (let [snapshot (swap! live-cursor-profile-state
                          (fn [state]
                            (let [state (update state :stats update-live-cursor-stat k nanos)]
                              (if (= k :highlight)
                                (update state :samples inc)
                                state))))]
      (when (and (= k :highlight)
                 (pos? (:samples snapshot))
                 (zero? (mod (:samples snapshot) live-cursor-profile-report-every)))
        (print-live-cursor-profile! snapshot)))))

(defn record-live-cursor-count! [k]
  (when (live-cursor-profile-enabled?)
    (swap! live-cursor-profile-state update-in [:stats k :n] (fnil inc 0))))

(def syntax-form-names
  #{"adsr" "and" "arpeggio" "arp" "asdr" "block" "by-scene" "choose" "chord" "clear" "clear-all"
    "cue" "d" "def" "delay" "distort" "euclid" "euclid-rot" "every-n"
    "filter" "gate-hold" "gate-seq" "gate_seq" "gs" "interleave" "map"
    "master-fx" "mute" "not" "offset" "or" "p" "pan" "phaser" "play-block"
    "play-note" "play-scene" "post-fx" "rand-range" "range" "repeat" "rev" "reverse" "reverb"
    "rotate" "s" "sample" "scale" "scene" "shape" "solo" "start!" "stop!" "take" "then" "times" "tracks" "transpose"
    "unmute" "unsolo"})

(defn syntax-attrs [^Color color]
  (doto (SimpleAttributeSet.)
    (StyleConstants/setForeground color)))

(def syntax-default-attrs (SimpleAttributeSet.))
(def syntax-comment-attrs (syntax-attrs (Color. 95 120 95)))
(def syntax-string-attrs (syntax-attrs (Color. 30 120 78)))
(def syntax-form-attrs (syntax-attrs (Color. 88 72 165)))
(def syntax-keyword-attrs (syntax-attrs (Color. 22 102 166)))
(def syntax-number-attrs (syntax-attrs (Color. 145 88 18)))
(def syntax-note-attrs (syntax-attrs (Color. 150 72 135)))

(def syntax-span-limit 1500)

(defn token-delimiter? [ch]
  (or (Character/isWhitespace ch)
      (contains? #{\( \) \[ \] \{ \} \" \;} ch)))

(defn syntax-kind [token]
  (cond
    (clojure.string/starts-with? token ":") :keyword
    (contains? syntax-form-names token) :form
    (re-matches #"[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:_[0-9]*)?" token) :number
    (re-matches #"(?i)[a-g](?:s|b)?-?[0-9]+" token) :note
    :else nil))

(defn syntax-spans [text]
  (loop [idx 0
         spans []]
    (if (or (>= idx (count text))
            (>= (count spans) syntax-span-limit))
      spans
      (let [ch (.charAt text idx)]
        (cond
          (= ch \;)
          (let [end (let [newline (.indexOf text "\n" idx)]
                      (if (neg? newline) (count text) newline))]
            (recur end (conj spans {:start idx :end end :kind :comment})))

          (= ch \")
          (let [end (loop [pos (inc idx)
                           escape? false]
                      (cond
                        (>= pos (count text)) (count text)
                        escape? (recur (inc pos) false)
                        (= (.charAt text pos) \\) (recur (inc pos) true)
                        (= (.charAt text pos) \") (inc pos)
                        :else (recur (inc pos) false)))]
            (recur end (conj spans {:start idx :end end :kind :string})))

          (token-delimiter? ch)
          (recur (inc idx) spans)

          :else
          (let [end (loop [pos idx]
                      (if (or (>= pos (count text))
                              (token-delimiter? (.charAt text pos)))
                        pos
                        (recur (inc pos))))
                token (subs text idx end)]
            (if-let [kind (syntax-kind token)]
              (recur end (conj spans {:start idx :end end :kind kind}))
              (recur end spans))))))))

(defn syntax-attrs-for-kind [kind]
  (case kind
    :comment syntax-comment-attrs
    :string syntax-string-attrs
    :form syntax-form-attrs
    :keyword syntax-keyword-attrs
    :number syntax-number-attrs
    :note syntax-note-attrs
    syntax-default-attrs))

(defn syntax-highlight-enabled? [text]
  (<= (count text) syntax-max-highlight-chars))

(defn refresh-syntax-colors! [^JTextComponent editor]
  (when (instance? JTextPane editor)
    (let [^StyledDocument doc (.getStyledDocument ^JTextPane editor)
          text (.getText editor)
          caret (.getCaretPosition editor)]
      (.putClientProperty editor syntax-refreshing-key true)
      (try
        (when (pos? (count text))
          (.setCharacterAttributes doc 0 (count text) syntax-default-attrs true)
          (when (syntax-highlight-enabled? text)
            (doseq [{:keys [start end kind]} (syntax-spans text)]
              (when (< start end)
                (.setCharacterAttributes doc start (- end start) (syntax-attrs-for-kind kind) false)))))
        (.setCaretPosition editor (min caret (count (.getText editor))))
        (finally
          (.putClientProperty editor syntax-refreshing-key false)
          (force-editor-repaint! editor))))))

(defn install-syntax-highlighter! [^JTextComponent editor]
  (let [timer (Timer. syntax-refresh-delay-ms nil)]
    (.setRepeats timer false)
    (.addActionListener
      timer
      (reify ActionListener
        (actionPerformed [_ _]
          (refresh-syntax-colors! editor))))
    (.addDocumentListener
      (.getDocument editor)
      (proxy [DocumentListener] []
        (insertUpdate [_]
          (when-not (.getClientProperty editor syntax-refreshing-key)
            (.restart timer)))
        (removeUpdate [_]
          (when-not (.getClientProperty editor syntax-refreshing-key)
            (.restart timer)))
        (changedUpdate [_] nil)))
    (SwingUtilities/invokeLater #(refresh-syntax-colors! editor))))

(defn editor-undo-manager [^JTextComponent editor]
  (.getClientProperty editor editor-undo-manager-key))

(defn clear-editor-undo-history! [^JTextComponent editor]
  (when-let [^UndoManager manager (editor-undo-manager editor)]
    (.discardAllEdits manager)))

(defn run-editor-undo! [^JTextComponent editor]
  (when-let [^UndoManager manager (editor-undo-manager editor)]
    (when (.canUndo manager)
      (.undo manager))))

(defn run-editor-redo! [^JTextComponent editor]
  (when-let [^UndoManager manager (editor-undo-manager editor)]
    (when (.canRedo manager)
      (.redo manager))))

(defn edit-menu-item [text enabled? f]
  (doto (JMenuItem. text)
    (.setEnabled (boolean enabled?))
    (.addActionListener
      (reify ActionListener
        (actionPerformed [_ _] (f))))))

(defn editor-context-menu [^JTextComponent editor]
  (let [selected? (boolean (seq (.getSelectedText editor)))
        editable? (and (.isEditable editor) (.isEnabled editor))
        ^UndoManager manager (editor-undo-manager editor)
        menu (JPopupMenu.)]
    (.add menu (edit-menu-item "Undo" (and manager (.canUndo manager))
                               #(run-editor-undo! editor)))
    (.add menu (edit-menu-item "Redo" (and manager (.canRedo manager))
                               #(run-editor-redo! editor)))
    (.addSeparator menu)
    (.add menu (edit-menu-item "Cut" (and editable? selected?) #(.cut editor)))
    (.add menu (edit-menu-item "Copy" selected? #(.copy editor)))
    (.add menu (edit-menu-item "Paste" editable? #(.paste editor)))
    (.addSeparator menu)
    (.add menu (edit-menu-item "Select All" (pos? (count (.getText editor)))
                               #(.selectAll editor)))
    menu))

(defn show-editor-context-menu! [^JTextComponent editor ^MouseEvent event]
  (when (.isPopupTrigger event)
    (when (and (not (seq (.getSelectedText editor)))
               (.isEnabled editor))
      (let [offset (.viewToModel editor (.getPoint event))]
        (when (>= offset 0)
          (.setCaretPosition editor offset))))
    (let [menu (editor-context-menu editor)]
      (.putClientProperty editor editor-context-menu-key menu)
      (.show menu editor (.getX event) (.getY event)))))

(defn bind-editor-action! [^JTextComponent editor keystroke action-key f]
  (.put (.getInputMap editor JComponent/WHEN_FOCUSED)
        (KeyStroke/getKeyStroke keystroke)
        action-key)
  (.put (.getActionMap editor)
        action-key
        (proxy [AbstractAction] []
          (actionPerformed [_] (f)))))

(defn install-standard-edit-controls! [^JTextComponent editor]
  (when-not (.getClientProperty editor editor-edit-controls-key)
    (let [manager (UndoManager.)]
      (.putClientProperty editor editor-undo-manager-key manager)
      (.addUndoableEditListener
        (.getDocument editor)
        (reify UndoableEditListener
          (undoableEditHappened [_ event]
            (when-not (.getClientProperty editor syntax-refreshing-key)
              (.addEdit manager (.getEdit event))))))
      (doseq [modifier ["control" "meta"]]
        (bind-editor-action! editor (str modifier " X") "glitchlisp-cut" #(.cut editor))
        (bind-editor-action! editor (str modifier " C") "glitchlisp-copy" #(.copy editor))
        (bind-editor-action! editor (str modifier " V") "glitchlisp-paste" #(.paste editor))
        (bind-editor-action! editor (str modifier " A") "glitchlisp-select-all" #(.selectAll editor))
        (bind-editor-action! editor (str modifier " Z") "glitchlisp-undo" #(run-editor-undo! editor))
        (bind-editor-action! editor (str modifier " Y") "glitchlisp-redo" #(run-editor-redo! editor))
        (bind-editor-action! editor (str "shift " modifier " Z") "glitchlisp-redo" #(run-editor-redo! editor)))
      (.addMouseListener
        editor
        (proxy [MouseAdapter] []
          (mousePressed [event] (show-editor-context-menu! editor event))
          (mouseReleased [event] (show-editor-context-menu! editor event))))
      (.putClientProperty editor editor-edit-controls-key true))))

(def live-step-fill-color
  (Color. 255 246 160 150))

(def live-step-border-color
  (Color. 230 190 55 210))

(def live-scene-fill-color
  (Color. 160 210 255 80))

(def live-scene-border-color
  (Color. 70 140 220 170))

(defn rect-intersects-clip? [graphics ^Rectangle rect]
  (let [^Rectangle clip (.getClipBounds graphics)]
    (or (nil? clip) (.intersects clip rect))))

(defn range-end-right [^JTextComponent editor text start end]
  (let [doc-length (.getLength (.getDocument editor))
        end-offset (max start (min end doc-length))
        end-rect (.modelToView editor end-offset)
        last-offset (when (< start end-offset) (dec end-offset))
        last-rect (when last-offset (.modelToView editor last-offset))
        last-char-width (if (and last-offset (< last-offset (count text)))
                          (.charWidth (.getFontMetrics editor (.getFont editor))
                                      (.charAt text last-offset))
                          0)]
    (max (if end-rect (.x ^Rectangle end-rect) 0)
         (if last-rect
           (+ (.x ^Rectangle last-rect) (max 1 last-char-width))
           0))))

(defn live-scene-segment-bounds [^JTextComponent editor start end]
  (when (< start end)
    (let [text (.getText editor)
          ^Rectangle start-rect (.modelToView editor start)
          right (range-end-right editor text start end)]
      (when start-rect
        (let [x (.x start-rect)
              y (.y start-rect)
              width (max 2 (- right x))
              height (.height start-rect)]
          (Rectangle. x y width height))))))

(defn paint-live-scene-segment! [^JTextComponent editor graphics start end]
  (when-let [^Rectangle rect (live-scene-segment-bounds editor start end)]
    (when (rect-intersects-clip? graphics rect)
      (.setColor graphics live-scene-fill-color)
      (.fillRect graphics (.x rect) (.y rect) (.width rect) (.height rect))
      (.setColor graphics live-scene-border-color)
      (.drawRect graphics (.x rect) (.y rect) (dec (.width rect)) (dec (.height rect))))))

(defn live-scene-range-segments [text [start end]]
  (let [length (count text)
        start (max 0 (min start length))
        end (max start (min end length))]
    (loop [line-start start
           segments []]
      (if (< line-start end)
        (let [newline (.indexOf text "\n" line-start)
              line-end (if (and (>= newline 0) (< newline end)) newline end)
              segment-end (if (= line-start line-end)
                            (min end (inc line-end))
                            line-end)
              segments (conj segments [line-start segment-end])]
          (if (and (>= newline 0) (< newline end))
            (recur (inc newline) segments)
            segments))
        segments))))

(defn paint-live-scene-range! [^JTextComponent editor graphics [start end]]
  (try
    (doseq [[start end] (live-scene-range-segments (.getText editor) [start end])]
      (paint-live-scene-segment! editor graphics start end))
    (catch Exception _)))

(defn live-step-range-bounds [^JTextComponent editor [start end]]
  (when (< start end)
    (try
      (let [text (.getText editor)
            ^Rectangle start-rect (.modelToView editor start)
            right (range-end-right editor text start end)]
        (when start-rect
          (let [x (.x start-rect)
                y (.y start-rect)
                width (max 2 (- right x))
                height (.height start-rect)]
            (Rectangle. x y width height))))
      (catch Exception _ nil))))

(defn paint-live-step-rect! [graphics ^Rectangle rect]
  (when (rect-intersects-clip? graphics rect)
    (.setColor graphics live-step-fill-color)
    (.fillRect graphics (.x rect) (.y rect) (.width rect) (.height rect))
    (.setColor graphics live-step-border-color)
    (.drawRect graphics (.x rect) (.y rect) (dec (.width rect)) (dec (.height rect)))))

(defn paint-live-step-range! [^JTextComponent editor graphics range]
  (when-let [rect (live-step-range-bounds editor range)]
    (paint-live-step-rect! graphics rect)))

(defn range-bounds [^JTextComponent editor [start end]]
  (when (< start end)
    (try
      (let [text (.getText editor)
            doc-length (.getLength (.getDocument editor))
            end-offset (max start (min end doc-length))
            ^Rectangle start-rect (.modelToView editor start)
            ^Rectangle end-rect (.modelToView editor end-offset)]
        (when (and start-rect end-rect)
          (let [x (min (.x start-rect) (.x end-rect))
                y (min (.y start-rect) (.y end-rect))
                right (max (+ (.x start-rect) (.width start-rect))
                           (range-end-right editor text start end))
                bottom (max (+ (.y start-rect) (.height start-rect))
                            (+ (.y end-rect) (.height end-rect)))]
            (Rectangle. x y (max 2 (- right x)) (max 2 (- bottom y))))))
      (catch Exception _ nil))))

(defn expanded-rect [^Rectangle rect]
  (when rect
    (Rectangle. (max 0 (- (.x rect) 2))
                (max 0 (- (.y rect) 2))
                (+ (.width rect) 4)
                (+ (.height rect) 4))))

(defn repaint-live-ranges! [^JTextComponent editor old-ranges new-ranges]
  (let [ranges (concat old-ranges new-ranges)
        rects (keep #(expanded-rect (range-bounds editor %)) ranges)]
    (cond
      (seq rects)
      (doseq [^Rectangle rect rects]
        (.repaint editor (.x rect) (.y rect) (.width rect) (.height rect))
        (.paintImmediately editor (.x rect) (.y rect) (.width rect) (.height rect)))

      (seq ranges)
      (do
        (.repaint editor)
        (.paintImmediately editor 0 0 (.getWidth editor) (.getHeight editor))))))

(defn live-overlay-active? [^JTextComponent editor]
  (or (.getClientProperty editor live-step-highlight-key)
      (.getClientProperty editor live-step-highlight-rects-key)
      (.getClientProperty editor live-scene-highlight-key)
      (.getClientProperty editor live-scene-highlight-segments-key)))

(defn repaint-current-live-overlay! [^JTextComponent editor]
  (when (live-overlay-active? editor)
    (let [step-ranges (or (.getClientProperty editor live-step-repaint-key)
                          (.getClientProperty editor live-step-highlight-key)
                          [])
          scene-range (.getClientProperty editor live-scene-highlight-key)
          scene-segments (.getClientProperty editor live-scene-highlight-segments-key)
          ranges (concat step-ranges
                         (when scene-range [scene-range])
                         (or scene-segments []))
          rects (concat
                  (or (.getClientProperty editor live-step-highlight-rects-key) [])
                  (keep #(expanded-rect (range-bounds editor %)) ranges))]
      (if (seq rects)
        (doseq [^Rectangle rect rects]
          (.repaint editor (.x rect) (.y rect) (.width rect) (.height rect)))
        (.repaint editor)))))

(defn install-live-repaint-pump! [^JTextComponent editor]
  (when-not (.getClientProperty editor live-repaint-pump-key)
    (let [timer (Timer. live-repaint-pump-delay-ms nil)]
      (.setRepeats timer true)
      (.addActionListener
        timer
        (reify ActionListener
          (actionPerformed [_ _]
            (repaint-current-live-overlay! editor))))
      (.putClientProperty editor live-repaint-pump-key timer)
      (.start timer))))

(defn paint-live-step-overlay! [^JTextComponent editor graphics]
  (let [profile? (live-cursor-profile-enabled?)
        start-ns (when profile? (System/nanoTime))]
    (try
      (if-let [segments (.getClientProperty editor live-scene-highlight-segments-key)]
        (doseq [[start end] segments]
          (paint-live-scene-segment! editor graphics start end))
        (when-let [range (.getClientProperty editor live-scene-highlight-key)]
          (paint-live-scene-range! editor graphics range)))
      (if-let [rects (.getClientProperty editor live-step-highlight-rects-key)]
        (doseq [^Rectangle rect rects]
          (paint-live-step-rect! graphics rect))
        (doseq [[start end] (or (.getClientProperty editor live-step-highlight-key) [])]
          (when (< start end)
            (paint-live-step-range! editor graphics [start end]))))
      (finally
        (when profile?
          (record-live-cursor-timing! :paint (- (System/nanoTime) start-ns)))))))

(defn editor-text-width [^JTextComponent editor]
  (let [metrics (.getFontMetrics editor (.getFont editor))]
    (+ 24
       (reduce max
               0
               (map #(.stringWidth metrics %)
                    (str/split-lines (.getText editor)))))))

(defn editor-pane []
  (let [editor (proxy [JTextPane] []
                 (getScrollableTracksViewportWidth []
                   (let [parent (.getParent this)]
                     (if (instance? JViewport parent)
                       (> (.getWidth ^JViewport parent)
                          (editor-text-width this))
                       false)))
                 (paintComponent [graphics]
                   (proxy-super paintComponent graphics)
                   (paint-live-step-overlay! this graphics)))]
    (.setCaret editor (safe-editor-caret))
    (install-standard-edit-controls! editor)
    (install-live-repaint-pump! editor)
    editor))

(defn clear-error-highlight! [^JTextComponent editor]
  (when-let [tag (.getClientProperty editor error-highlight-key)]
    (.removeHighlight (.getHighlighter editor) tag)
    (.putClientProperty editor error-highlight-key nil)))

(defn clear-paren-highlight! [^JTextComponent editor]
  (doseq [tag (or (.getClientProperty editor paren-highlight-key) [])]
    (.removeHighlight (.getHighlighter editor) tag))
  (.putClientProperty editor paren-highlight-key nil))

(defn clear-live-step-highlight! [^JTextComponent editor]
  (when (or (.getClientProperty editor live-step-highlight-key)
            (.getClientProperty editor live-scene-highlight-key))
    (let [old-ranges (concat
                       (or (.getClientProperty editor live-step-highlight-key) [])
                       (when-let [scene-range (.getClientProperty editor live-scene-highlight-key)]
                         [scene-range]))]
    (.putClientProperty editor live-step-highlight-key nil)
      (.putClientProperty editor live-step-highlight-rects-key nil)
      (.putClientProperty editor live-scene-highlight-key nil)
      (.putClientProperty editor live-scene-highlight-segments-key nil)
      (.putClientProperty editor live-step-repaint-key nil)
      (repaint-live-ranges! editor old-ranges []))))

(defn top-level-vector-cell-ranges [text open-offset close-offset]
  (loop [idx (inc open-offset)
         depth 0
         cell-start nil
         ranges []
         in-string? false
         escape? false
         in-comment? false]
    (if (>= idx close-offset)
      (cond-> ranges
        cell-start (conj [cell-start close-offset]))
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) depth cell-start ranges in-string? false (not= ch \newline))

          escape?
          (recur (inc idx) depth cell-start ranges in-string? false false)

          (and in-string? (= ch \\))
          (recur (inc idx) depth cell-start ranges true true false)

          (= ch \")
          (recur (inc idx) depth (or cell-start idx) ranges (not in-string?) false false)

          in-string?
          (recur (inc idx) depth cell-start ranges true false false)

          (= ch \;)
          (recur (inc idx) depth cell-start ranges false false true)

          (and (zero? depth) (Character/isWhitespace ch))
          (if cell-start
            (recur (inc idx) depth nil (conj ranges [cell-start idx]) false false false)
            (recur (inc idx) depth nil ranges false false false))

          (#{\( \[} ch)
          (recur (inc idx) (inc depth) (or cell-start idx) ranges false false false)

          (#{\) \]} ch)
          (recur (inc idx) (max 0 (dec depth)) (or cell-start idx) ranges false false false)

          :else
          (recur (inc idx) depth (or cell-start idx) ranges false false false))))))

(defn skip-space-and-comments [text idx limit]
  (loop [idx idx
         in-comment? false]
    (if (>= idx limit)
      idx
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) (not= ch \newline))

          (= ch \;)
          (recur (inc idx) true)

          (Character/isWhitespace ch)
          (recur (inc idx) false)

          :else idx)))))

(defn token-char? [ch]
  (not (or (Character/isWhitespace ^char ch)
           (contains? #{\( \) \[ \] \;} ch))))

(defn token-range [text idx limit]
  (let [idx (skip-space-and-comments text idx limit)]
    (when (< idx limit)
      (loop [end idx]
        (if (and (< end limit) (token-char? (.charAt text end)))
          (recur (inc end))
          [idx end])))))

(defn token-text [text idx limit]
  (when-let [[start end] (token-range text idx limit)]
    (subs text start end)))

(defn form-end-offset [text idx limit]
  (let [idx (skip-space-and-comments text idx limit)]
    (when (< idx limit)
      (let [ch (.charAt text idx)]
        (cond
          (= ch \()
          (matching-close text idx \( \))

          (= ch \[)
          (matching-close text idx \[ \])

          :else
          (second (token-range text idx limit)))))))

(declare enclosing-track-range)
(declare code-visible-text)
(declare gate-pattern-range-entry)

(def live-pattern-parameter-names
  #{":gate" ":note" ":dur" ":amp" ":detune-cents" ":detune" ":phase"
    ":pulse-width" ":pw" ":morph" ":gain" ":unison" ":unison-detune"
    ":unison-spread" ":fm-ratio" ":fm-depth"})

(defn repeat-cells [cells n]
  (vec (apply concat (repeat (max 0 n) cells))))

(defn parse-count-token [token]
  (try
    (Integer/parseInt (or token "0"))
    (catch Exception _ 0)))

(defn list-head-and-args [text open close]
  (let [head-range (token-range text (inc open) close)]
    (when head-range
      {:head (subs text (first head-range) (second head-range))
       :args-start (second head-range)})))

(defn p-range-entry [text args-start close]
  (let [first-idx (skip-space-and-comments text args-start close)
        first-token (token-text text first-idx close)]
    (if (= first-token ":repeat")
      (let [count-range (token-range text (+ first-idx (count first-token)) close)
            count-token (when count-range (subs text (first count-range) (second count-range)))
            pattern-start (if count-range (second count-range) first-idx)
            entry (gate-pattern-range-entry text pattern-start close)]
        (when entry
          {:cells (repeat-cells (:cells entry) (parse-count-token count-token))
           :loop-start 0}))
      (gate-pattern-range-entry text first-idx close))))

(defn times-range-entry [text open args-start close]
  (let [count-range (token-range text args-start close)
        count-token (when count-range (subs text (first count-range) (second count-range)))
        pattern-start (if count-range (second count-range) args-start)
        entry (gate-pattern-range-entry text pattern-start close)]
    (when entry
      {:cells (repeat-cells [[open (inc close)]] (parse-count-token count-token))
       :loop-start 0})))

(defn then-range-entry [text args-start close]
  (loop [idx args-start
         stages []]
    (let [idx (skip-space-and-comments text idx close)]
      (if (>= idx close)
        (when (seq stages)
          (let [prefix (vec (apply concat (map :cells (butlast stages))))
                final-stage (last stages)
                final-cells (:cells final-stage)]
            {:cells (into prefix final-cells)
             :loop-start (if (= (:head final-stage) "times")
                           0
                           (count prefix))}))
        (let [end (form-end-offset text idx close)
              entry (gate-pattern-range-entry text idx (or end close))]
          (if (and end entry)
            (recur (inc end) (conj stages entry))
            (recur (inc idx) stages)))))))

(defn gate-pattern-range-entry [text idx limit]
  (let [idx (skip-space-and-comments text idx limit)]
    (when (< idx limit)
      (let [ch (.charAt text idx)]
        (cond
          (= ch \[)
          (when-let [close (matching-close text idx \[ \])]
            {:cells (top-level-vector-cell-ranges text idx close)
             :loop-start 0})

          (= ch \()
          (when-let [close (matching-close text idx \( \))]
            (when-let [{:keys [head args-start]} (list-head-and-args text idx close)]
              (case head
                "p" (p-range-entry text args-start close)
                "then" (then-range-entry text args-start close)
                "times" (when-let [entry (times-range-entry text idx args-start close)]
                          (assoc entry :head "times"))
                nil)))

          :else nil)))))

(defn gate-pattern-vector-ranges [text]
  (let [visible (code-visible-text text)]
    (loop [idx 0
           ranges []]
      (if-let [gate-idx (let [found (.indexOf visible ":gate" idx)]
                          (when (>= found 0) found))]
        (let [pattern-start (skip-space-and-comments text (+ gate-idx 5) (count text))
              pattern-end (form-end-offset text pattern-start (count text))
              entry (when pattern-end
                      (gate-pattern-range-entry text pattern-start pattern-end))]
          (if entry
            (recur (inc pattern-end) (conj ranges (assoc entry :gate-idx gate-idx)))
            (recur (+ gate-idx 5) ranges)))
        ranges))))

(defn live-pattern-vector-ranges [text]
  (let [visible (code-visible-text text)
        text-length (count text)]
    (loop [idx 0
           ranges []]
      (if-let [param-idx (loop [search-idx idx
                                best nil]
                           (if-let [[param found] (->> live-pattern-parameter-names
                                                       (keep (fn [param]
                                                               (let [found (.indexOf visible param search-idx)]
                                                                 (when (>= found 0)
                                                                   [param found]))))
                                                       (sort-by second)
                                                       first)]
                             [param found]
                             best))]
        (let [[param found] param-idx
              pattern-start (skip-space-and-comments text (+ found (count param)) text-length)
              pattern-end (form-end-offset text pattern-start text-length)
              entry (when pattern-end
                      (gate-pattern-range-entry text pattern-start pattern-end))]
          (if entry
            (recur (inc pattern-end)
                   (conj ranges (assoc entry
                                       :gate-idx found
                                       :pattern-idx found
                                       :param param)))
            (recur (+ found (count param)) ranges)))
        ranges))))

(defn scene-form-range-by-id [text scene]
  (when scene
    (let [visible (code-visible-text text)
          needle1 (str "(scene :" scene)
          needle2 (str "(block :" scene)]
      (loop [idx 0]
        (let [scene-idx (.indexOf visible needle1 idx)
              block-idx (.indexOf visible needle2 idx)
              start (cond
                      (and (>= scene-idx 0) (>= block-idx 0)) (min scene-idx block-idx)
                      (>= scene-idx 0) scene-idx
                      (>= block-idx 0) block-idx
                      :else -1)]
          (when (>= start 0)
            (if-let [end (matching-close text start \( \))]
              [start end]
              (recur (inc start)))))))))

(defn scene-header-range-by-id [text scene]
  (when-let [[start end] (scene-form-range-by-id text scene)]
    (let [line-end (let [found (.indexOf text "\n" start)]
                     (if (and (>= found 0) (< found end)) found (min end (+ start 80))))]
      [start line-end])))

(defn enclosing-def-range [text caret]
  (let [caret (max 0 (min caret (count text)))
        visible (code-visible-text text)]
    (loop [idx (.lastIndexOf visible "(def" caret)]
      (when (>= idx 0)
        (if-let [end (matching-close text idx \( \))]
          (if (<= idx caret end)
            [idx end]
            (recur (.lastIndexOf visible "(def" (max 0 (dec idx)))))
          (recur (.lastIndexOf visible "(def" (max 0 (dec idx)))))))))

(defn def-name-range-at [text idx]
  (when-let [[start end] (enclosing-def-range text idx)]
    (let [name-range (token-range text (+ start 4) end)]
      (when name-range
        [(subs text (first name-range) (second name-range)) start end]))))

(defn code-visible-text [text]
  (let [builder (StringBuilder.)]
    (loop [idx 0
           in-string? false
           escape? false
           in-comment? false]
      (if (< idx (count text))
        (let [ch (.charAt text idx)]
          (cond
            in-comment?
            (do
              (.append builder (if (= ch \newline) \newline \space))
              (recur (inc idx) false false (not= ch \newline)))

            escape?
            (do
              (.append builder \space)
              (recur (inc idx) in-string? false false))

            (and in-string? (= ch \\))
            (do
              (.append builder \space)
              (recur (inc idx) true true false))

            (= ch \")
            (do
              (.append builder \space)
              (recur (inc idx) (not in-string?) false false))

            in-string?
            (do
              (.append builder (if (= ch \newline) \newline \space))
              (recur (inc idx) true false false))

            (= ch \;)
            (do
              (.append builder \space)
              (recur (inc idx) false false true))

            :else
            (do
              (.append builder ch)
              (recur (inc idx) false false false))))
        (str builder)))))

(defn symbol-mention-pattern [symbol-name]
  (re-pattern (str "(^|[^A-Za-z0-9_!?.*/+<>=-])"
                   (java.util.regex.Pattern/quote symbol-name)
                   "([^A-Za-z0-9_!?.*/+<>=-]|$)")))

(defn symbol-mentioned-in-visible-text? [visible-text symbol-name]
  (boolean (re-find (symbol-mention-pattern symbol-name) visible-text)))

(defn symbol-mentioned-in-range? [text [start end] symbol-name]
  (symbol-mentioned-in-visible-text?
    (code-visible-text (subs text start end))
    symbol-name))

(def symbol-token-pattern #"[A-Za-z0-9_!?.*/+<>=-]+")

(defn visible-symbol-set [visible-text]
  (set (re-seq symbol-token-pattern visible-text)))

(defn scene-membership-context [text scene-range]
  (when scene-range
    (let [visible-text (code-visible-text (subs text (first scene-range) (second scene-range)))]
      {:range scene-range
       :symbols (visible-symbol-set visible-text)})))

(defn active-gate-entry-in-scene-context? [text scene-context entry]
  (if scene-context
    (let [scene-range (:range scene-context)
          gate-idx (:gate-idx entry)]
      (or (<= (first scene-range) gate-idx (second scene-range))
          (when-let [[def-name _ _] (def-name-range-at text gate-idx)]
            (contains? (:symbols scene-context) def-name))))
    true))

(defn active-gate-entry-in-scene-range? [text scene-range entry]
  (active-gate-entry-in-scene-context? text (scene-membership-context text scene-range) entry))

(defn active-gate-entry? [text scene entry]
  (active-gate-entry-in-scene-range? text (scene-form-range-by-id text scene) entry))

(defn focused-gate-range [text caret]
  (or (enclosing-track-range text caret)
      (when-let [[_ start end] (def-name-range-at text caret)]
        [start end])))

(defn cached-focused-gate-range [^JTextComponent editor text]
  (let [caret (.getCaretPosition editor)]
    (if (= caret (.getClientProperty editor live-focus-caret-key))
      (.getClientProperty editor live-focus-range-key)
      (let [focus-range (focused-gate-range text caret)]
        (.putClientProperty editor live-focus-caret-key caret)
        (.putClientProperty editor live-focus-range-key focus-range)
        focus-range))))

(defn focused-gate-entry? [focus-range entry]
  (if focus-range
    (let [gate-idx (:gate-idx entry)]
      (<= (first focus-range) gate-idx (second focus-range)))
    true))

(defn cached-live-highlight-text [^JTextComponent editor]
  (or (.getClientProperty editor live-gate-ranges-text-key)
      (let [text (.getText editor)]
        (.putClientProperty editor live-gate-ranges-text-key text)
        text)))

(defn cached-gate-pattern-vector-ranges [^JTextComponent editor]
  (if-let [ranges (.getClientProperty editor live-gate-ranges-key)]
    ranges
    (let [text (cached-live-highlight-text editor)
          ranges (live-pattern-vector-ranges text)]
      (.putClientProperty editor live-gate-ranges-key ranges)
      ranges)))

(defn cached-scene-form-range-by-id [^JTextComponent editor text scene]
  (when scene
    (let [cache (or (.getClientProperty editor live-scene-ranges-key) {})]
      (if (contains? cache scene)
        (get cache scene)
        (let [scene-range (scene-form-range-by-id text scene)]
          (.putClientProperty editor live-scene-ranges-key (assoc cache scene scene-range))
          scene-range)))))

(defn cached-scene-membership-context [^JTextComponent editor text scene scene-range]
  (when (and scene scene-range)
    (let [cache (or (.getClientProperty editor live-scene-contexts-key) {})]
      (if (contains? cache scene)
        (get cache scene)
        (let [context (scene-membership-context text scene-range)]
          (.putClientProperty editor live-scene-contexts-key (assoc cache scene context))
          context)))))

(defn cached-active-gate-entries [^JTextComponent editor text scene scene-context]
  (let [entries (cached-gate-pattern-vector-ranges editor)]
    (if scene
      (let [cache (or (.getClientProperty editor live-active-gate-entries-key) {})]
        (if (contains? cache scene)
          (get cache scene)
          (let [active-entries (vec (filter #(active-gate-entry-in-scene-context?
                                               text scene-context %)
                                            entries))]
            (.putClientProperty editor live-active-gate-entries-key
                                (assoc cache scene active-entries))
            active-entries)))
      entries)))

(defn cached-live-scene-range-segments [^JTextComponent editor text scene scene-range]
  (when (and scene scene-range)
    (let [cache (or (.getClientProperty editor live-scene-segments-cache-key) {})]
      (if (contains? cache scene)
        (get cache scene)
        (let [segments (live-scene-range-segments text scene-range)]
          (.putClientProperty editor live-scene-segments-cache-key
                              (assoc cache scene segments))
          segments)))))

(defn focused-active-gate-cache-key [scene focus-range]
  [(or scene ::global) focus-range])

(defn cached-focused-active-gate-entries [^JTextComponent editor scene focus-range active-entries]
  (let [cache-key (focused-active-gate-cache-key scene focus-range)
        cache (or (.getClientProperty editor live-focused-active-gate-entries-key) {})]
    (if (contains? cache cache-key)
      (get cache cache-key)
      (let [focused-entries (vec (filter #(focused-gate-entry? focus-range %)
                                         active-entries))]
        (.putClientProperty editor live-focused-active-gate-entries-key
                            (assoc cache cache-key focused-entries))
        focused-entries))))

(defn gcd-long [a b]
  (loop [a (abs (long a))
         b (abs (long b))]
    (if (zero? b)
      a
      (recur b (mod a b)))))

(defn lcm-long [a b]
  (if (or (zero? a) (zero? b))
    0
    (let [g (gcd-long a b)]
      (* (quot a g) b))))

(defn entry-loop-start [entry]
  (let [cells (if (map? entry) (:cells entry) entry)]
    (if (seq cells)
      (min (long (if (map? entry) (:loop-start entry) 0))
           (dec (count cells)))
      0)))

(defn entry-loop-length [entry]
  (let [cells (if (map? entry) (:cells entry) entry)
        loop-start (entry-loop-start entry)]
    (max 1 (- (count cells) loop-start))))

(defn normalized-live-step [step entries]
  (let [threshold (reduce max 0 (map entry-loop-start entries))
        period (reduce lcm-long 1 (map entry-loop-length entries))]
    (if (< step threshold)
      step
      (+ threshold (mod (- step threshold) (max 1 period))))))

(defn resolve-live-step-ranges [step entries]
  (vec
    (keep (fn [entry]
            (let [cells (if (map? entry) (:cells entry) entry)
                  loop-start (if (map? entry) (:loop-start entry) 0)]
              (when (seq cells)
                (let [loop-start (min loop-start (dec (count cells)))
                      idx (if (< step loop-start)
                            step
                            (+ loop-start
                               (mod (- step loop-start)
                                    (max 1 (- (count cells) loop-start)))))
                      [start end] (nth cells idx)]
                  (when (< start end)
                    [start end])))))
          entries)))

(defn bounded-cache-assoc [cache k v limit]
  (let [cache (or cache {:values {} :order []})
        values (or (:values cache) {})
        order (or (:order cache) [])
        existing? (contains? values k)
        values (assoc values k v)
        order (if existing? order (conj order k))]
    (if (<= (count values) limit)
      {:values values :order order}
      (let [drop-key (first order)]
        {:values (dissoc values drop-key)
         :order (subvec (vec order) 1)}))))

(defn cached-live-step-ranges [^JTextComponent editor scene focus-range step focused-entries]
  (let [normalized-step (normalized-live-step step focused-entries)
        cache-key [(or scene ::global) focus-range normalized-step]
        cache (.getClientProperty editor live-resolved-step-ranges-key)
        values (:values cache)]
    (if (contains? values cache-key)
      (get values cache-key)
      (let [ranges (resolve-live-step-ranges normalized-step focused-entries)]
        (.putClientProperty editor live-resolved-step-ranges-key
                            (bounded-cache-assoc cache cache-key ranges live-resolved-step-ranges-limit))
        ranges))))

(defn clear-live-gate-range-cache! [^JTextComponent editor]
  (.putClientProperty editor live-gate-ranges-text-key nil)
  (.putClientProperty editor live-gate-ranges-key nil)
  (.putClientProperty editor live-step-highlight-rects-key nil)
  (.putClientProperty editor live-focus-caret-key nil)
  (.putClientProperty editor live-focus-range-key nil)
  (.putClientProperty editor live-scene-ranges-key nil)
  (.putClientProperty editor live-scene-contexts-key nil)
  (.putClientProperty editor live-active-gate-entries-key nil)
  (.putClientProperty editor live-focused-active-gate-entries-key nil)
  (.putClientProperty editor live-resolved-step-ranges-key nil)
  (.putClientProperty editor live-scene-segments-cache-key nil)
  (.putClientProperty editor live-scene-highlight-segments-key nil))

(defn install-live-gate-range-cache! [^JTextComponent editor]
  (.addDocumentListener
    (.getDocument editor)
    (proxy [DocumentListener] []
      (insertUpdate [_] (clear-live-gate-range-cache! editor))
      (removeUpdate [_] (clear-live-gate-range-cache! editor))
      (changedUpdate [_] nil))))

(defn highlight-live-step! [^JTextComponent editor step scene]
  (let [text (cached-live-highlight-text editor)
        focus-range (cached-focused-gate-range editor text)
        old-step-ranges (or (.getClientProperty editor live-step-repaint-key)
                            (.getClientProperty editor live-step-highlight-key)
                            [])
        old-scene-range (.getClientProperty editor live-scene-highlight-key)
        scene-form-range (cached-scene-form-range-by-id editor text scene)
        scene-context (cached-scene-membership-context editor text scene scene-form-range)
        scene-range (when-let [[start end] scene-form-range]
                      [start (inc end)])
        scene-segments (cached-live-scene-range-segments editor text scene scene-range)
        active-entries (cached-active-gate-entries editor text scene scene-context)
        focused-entries (cached-focused-active-gate-entries editor scene focus-range active-entries)
        ranges (cached-live-step-ranges editor scene focus-range step focused-entries)
        scene-ranges-to-repaint (when (not= old-scene-range scene-range)
                                  (concat (when old-scene-range [old-scene-range])
                                          (when scene-range [scene-range])))
        ranges-changed? (not= old-step-ranges ranges)
        scene-changed? (not= old-scene-range scene-range)
        step-rects (if ranges-changed?
                     (let [rects (vec (keep #(live-step-range-bounds editor %) ranges))]
                       (when (= (count rects) (count ranges))
                         rects))
                     (.getClientProperty editor live-step-highlight-rects-key))]
    (.putClientProperty editor live-step-highlight-key ranges)
    (.putClientProperty editor live-step-highlight-rects-key step-rects)
    (.putClientProperty editor live-scene-highlight-key scene-range)
    (.putClientProperty editor live-scene-highlight-segments-key scene-segments)
    (.putClientProperty editor live-step-repaint-key ranges)
    (if (or (seq ranges) scene-range)
      (when (or ranges-changed? scene-changed?)
        (repaint-live-ranges! editor
                              (concat old-step-ranges scene-ranges-to-repaint)
                              ranges))
      (clear-live-step-highlight! editor))))

(defn queue-live-step-highlight!
  ([^JTextComponent editor step]
   (queue-live-step-highlight! editor step nil))
  ([^JTextComponent editor step scene]
  (let [should-schedule? (atom false)]
    (swap! shared/state
           (fn [current]
             (let [current (assoc current
                                  :live-highlight-step step
                                  :live-highlight-scene scene)]
               (if (:live-highlight-scheduled current)
                 current
                 (do
                   (reset! should-schedule? true)
                   (assoc current :live-highlight-scheduled true))))))
    (when @should-schedule?
      (SwingUtilities/invokeLater
        #(let [step (:live-highlight-step @shared/state)
               scene (:live-highlight-scene @shared/state)]
           (swap! shared/state assoc :live-highlight-scheduled false)
           (when step
             (highlight-live-step! editor step scene))))))))

(defn queue-current-live-step-highlight!
  ([^JTextComponent editor]
   (queue-current-live-step-highlight! editor nil))
  ([^JTextComponent editor received-ns]
   (let [scheduled-ns (System/nanoTime)
         should-schedule? (atom false)]
     (swap! shared/state
            (fn [current]
              (if (:live-highlight-scheduled current)
                (do
                  (record-live-cursor-count! :coalesced)
                  current)
                (do
                  (reset! should-schedule? true)
                  (assoc current
                         :live-highlight-scheduled true
                         :live-highlight-received-ns received-ns
                         :live-highlight-scheduled-ns scheduled-ns)))))
     (when @should-schedule?
       (SwingUtilities/invokeLater
         (fn []
           (let [edt-start-ns (System/nanoTime)
                 state @shared/state
                 step (:live-highlight-step state)
                 scene (:live-highlight-scene state)
                 queued-received-ns (:live-highlight-received-ns state)
                 queued-scheduled-ns (:live-highlight-scheduled-ns state)]
             (swap! shared/state assoc :live-highlight-scheduled false)
             (when queued-received-ns
               (record-live-cursor-timing! :receive-to-edt (- edt-start-ns queued-received-ns)))
             (when queued-scheduled-ns
               (record-live-cursor-timing! :queue (- edt-start-ns queued-scheduled-ns)))
             (when step
               (let [highlight-start-ns (System/nanoTime)]
                 (highlight-live-step! editor step scene)
                 (record-live-cursor-timing!
                   :highlight
                   (- (System/nanoTime) highlight-start-ns)))))))))))

(defn highlight-editor-range! [^JTextComponent editor start end]
  (let [text (.getText editor)
        start (max 0 (min start (count text)))
        end (max start (min end (count text)))]
    (clear-error-highlight! editor)
    (.putClientProperty
      editor
      error-highlight-key
      (.addHighlight (.getHighlighter editor)
                     start
                     (max (inc start) end)
                     error-highlight-painter))
    (.setCaretPosition editor start)
    (.select editor start (max (inc start) end))
    (when-let [rect (.modelToView editor start)]
      (.scrollRectToVisible editor rect))))

(defn matching-open [text close-index]
  (loop [idx 0
         stack []
         in-string? false
         escape? false
         in-comment? false]
    (when (<= idx close-index)
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) stack in-string? false (not= ch \newline))

          escape?
          (recur (inc idx) stack in-string? false false)

          (and in-string? (= ch \\))
          (recur (inc idx) stack true true false)

          (= ch \")
          (recur (inc idx) stack (not in-string?) false false)

          in-string?
          (recur (inc idx) stack true false false)

          (= ch \;)
          (recur (inc idx) stack false false true)

          (contains? opening-delimiters ch)
          (recur (inc idx) (conj stack {:ch ch :offset idx}) false false false)

          (contains? closing-delimiters ch)
          (let [expected-open (closing-delimiters ch)
                opener (peek stack)]
            (if (and opener (= (:ch opener) expected-open))
              (if (= idx close-index)
                (:offset opener)
                (recur (inc idx) (pop stack) false false false))
              nil))

          :else
          (recur (inc idx) stack false false false))))))

(defn delimiter-match-range [text offset]
  (when (and (<= 0 offset) (< offset (count text)))
    (let [ch (.charAt text offset)]
      (cond
        (contains? opening-delimiters ch)
        (when-let [close (matching-close text offset ch (opening-delimiters ch))]
          [offset close])

        (contains? closing-delimiters ch)
        (when-let [open (matching-open text offset)]
          [open offset])))))

(defn delimiter-offset-near-caret [text caret]
  (let [caret (max 0 (min caret (count text)))]
    (or (when (pos? caret)
          (let [idx (dec caret)
                ch (.charAt text idx)]
            (when (contains? closing-delimiters ch)
              idx)))
        (when (< caret (count text))
          (let [ch (.charAt text caret)]
            (when (or (contains? opening-delimiters ch)
                      (contains? closing-delimiters ch))
              caret)))
        (when (pos? caret)
          (let [idx (dec caret)
                ch (.charAt text idx)]
            (when (contains? opening-delimiters ch)
              idx))))))

(defn refresh-paren-highlight! [^JTextComponent editor]
  (clear-paren-highlight! editor)
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (when-let [offset (delimiter-offset-near-caret text caret)]
      (let [highlighter (.getHighlighter editor)
            ch (.charAt text offset)]
        (if-let [[open close] (delimiter-match-range text offset)]
          (.putClientProperty
            editor
            paren-highlight-key
            [(.addHighlight highlighter open (inc open) paren-peer-painter)
             (.addHighlight highlighter close (inc close) paren-match-painter)])
          (when (contains? closing-delimiters ch)
            (.putClientProperty
              editor
              paren-highlight-key
              [(.addHighlight highlighter offset (inc offset) paren-unmatched-painter)])))))))

(defn schedule-paren-highlight-refresh! [^JTextComponent editor]
  (if-let [^Timer timer (.getClientProperty editor paren-refresh-timer-key)]
    (.restart timer)
    (let [timer (Timer. paren-refresh-delay-ms nil)]
      (.setRepeats timer false)
      (.addActionListener
        timer
        (reify ActionListener
          (actionPerformed [_ _]
            (refresh-paren-highlight! editor))))
      (.putClientProperty editor paren-refresh-timer-key timer)
      (.start timer))))

(defn install-paren-highlighter! [^JTextComponent editor]
  (.addCaretListener
    editor
    (reify CaretListener
      (caretUpdate [_ _]
        (schedule-paren-highlight-refresh! editor))))
  (.addDocumentListener
    (.getDocument editor)
    (proxy [DocumentListener] []
      (insertUpdate [_] (schedule-paren-highlight-refresh! editor))
      (removeUpdate [_] (schedule-paren-highlight-refresh! editor))
      (changedUpdate [_] nil))))

(defn live-number-or-note-error? [message]
  (boolean (and message
                (re-find #"(?i)expected number or note" message))))

(defn pattern-value-range [text param]
  (let [visible (code-visible-text text)]
    (loop [idx 0]
      (when-let [found (let [found (.indexOf visible param idx)]
                         (when (>= found 0) found))]
        (let [before-ok? (or (zero? found)
                             (Character/isWhitespace (.charAt visible (dec found)))
                             (contains? #{\( \[} (.charAt visible (dec found))))
              after (+ found (count param))
              after-ok? (or (= after (count visible))
                            (Character/isWhitespace (.charAt visible after))
                            (contains? #{\) \]} (.charAt visible after)))]
          (if (and before-ok? after-ok?)
            (let [start (skip-space-and-comments text after (count text))
                  end (form-end-offset text start (count text))]
              (when end [start end]))
            (recur (+ found (count param)))))))))

(defn pattern-value-ranges [text param]
  (let [visible (code-visible-text text)]
    (loop [idx 0
           ranges []]
      (if-let [found (let [found (.indexOf visible param idx)]
                       (when (>= found 0) found))]
        (let [before-ok? (or (zero? found)
                             (Character/isWhitespace (.charAt visible (dec found)))
                             (contains? #{\( \[} (.charAt visible (dec found))))
              after (+ found (count param))
              after-ok? (or (= after (count visible))
                            (Character/isWhitespace (.charAt visible after))
                            (contains? #{\) \]} (.charAt visible after)))]
          (if (and before-ok? after-ok?)
            (let [start (skip-space-and-comments text after (count text))
                  end (form-end-offset text start (count text))]
              (recur after (cond-> ranges end (conj [start end]))))
            (recur after ranges)))
        ranges))))

(defn vector-close-range [text open limit]
  (when-let [close (matching-close text open \[ \])]
    (when (<= close limit)
      [open (inc close)])))

(defn first-vector-deeper-than [text [start end] allowed-depth]
  (loop [idx start
         vector-depth 0
         in-string? false
         escape? false
         in-comment? false]
    (when (< idx end)
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) vector-depth in-string? false (not= ch \newline))

          escape?
          (recur (inc idx) vector-depth in-string? false false)

          (and in-string? (= ch \\))
          (recur (inc idx) vector-depth true true false)

          (= ch \")
          (recur (inc idx) vector-depth (not in-string?) false false)

          in-string?
          (recur (inc idx) vector-depth true false false)

          (= ch \;)
          (recur (inc idx) vector-depth false false true)

          (= ch \[)
          (let [next-depth (inc vector-depth)]
            (if (> next-depth allowed-depth)
              (vector-close-range text idx end)
              (recur (inc idx) next-depth false false false)))

          (= ch \])
          (recur (inc idx) (max 0 (dec vector-depth)) false false false)

          :else
          (recur (inc idx) vector-depth false false false))))))

(defn source-diagnostic-for-message [source message]
  (when (live-number-or-note-error? message)
    (or
      (when-let [range (some #(first-vector-deeper-than source % 2)
                             (pattern-value-ranges source ":note"))]
          {:range range
           :message (str "expected number or note in :note pattern; "
                         "nested vectors are not valid here. Use [e3 f3] for a chord, not [[e3 f3]].")})
      (when-let [range (some #(first-vector-deeper-than source % 1)
                             (pattern-value-ranges source ":sample-data"))]
          {:range range
           :message "expected number or note in :sample-data; sample data cells must be numbers or notes"}))))

(defn source-error-exception [source message]
  (if-let [{:keys [range message]} (source-diagnostic-for-message source message)]
    (let [[start end] range
          {:keys [line column]} (line-column-for-offset source start)]
      (ex-info message
               {:offset start
                :end-offset end
                :line line
                :column column}))
    (ex-info message {})))

(defn error-offset [^JTextComponent editor ex]
  (let [data (ex-data ex)
        text (.getText editor)]
    (or (:offset data)
        (when (and (:line data) (:column data))
          (offset-for-line-column text (:line data) (:column data)))
        (when-let [{:keys [line column]} (line-column-from-message (.getMessage ex))]
          (offset-for-line-column text line column)))))

(defn error-range [^JTextComponent editor ex]
  (let [data (ex-data ex)
        text (.getText editor)]
    (when-let [start (error-offset editor ex)]
      [start (or (:end-offset data)
                 (:end data)
                 (min (count text) (inc start)))])))

(defn focus-source-error! [^JTextComponent editor ^JLabel status ex]
  (when-let [[start end] (error-range editor ex)]
    (let [text (.getText editor)
          start (max 0 (min start (count text)))
          end (max start (min end (count text)))]
      (.requestFocusInWindow editor)
      (highlight-editor-range! editor start end)
      (set-status! status (clean-error-message ex)))))

(defn report-source-error! [^JTextComponent editor ^JLabel status ex]
  (focus-source-error! editor status ex)
  (set-status! status (clean-error-message ex)))

(defn text-line-count [text]
  (max 1 (inc (count (filter #(= % \newline) text)))))

(defn line-number-text [^JTextComponent editor]
  (let [line-count (text-line-count (.getText editor))
        width (count (str line-count))]
    (->> (range 1 (inc line-count))
         (map #(format (str "%" width "d ") %))
         (clojure.string/join "\n"))))

(defn refresh-line-numbers! [^JTextComponent editor ^JTextComponent line-numbers]
  (.setText line-numbers (line-number-text editor)))

(defn line-number-gutter [^JTextComponent editor]
  (let [line-numbers (JTextPane.)]
    (.setEditable line-numbers false)
    (.setFocusable line-numbers false)
    (.setFont line-numbers (.getFont editor))
    (.setOpaque line-numbers true)
    (.setBackground line-numbers (Color. 242 242 242))
    (.setForeground line-numbers (Color. 105 105 105))
    (.setBorder line-numbers (BorderFactory/createEmptyBorder 0 4 0 6))
    (refresh-line-numbers! editor line-numbers)
    (.addDocumentListener
      (.getDocument editor)
      (proxy [DocumentListener] []
        (insertUpdate [_]
          (clear-error-highlight! editor)
          (refresh-line-numbers! editor line-numbers))
        (removeUpdate [_]
          (clear-error-highlight! editor)
          (refresh-line-numbers! editor line-numbers))
        (changedUpdate [_] nil)))
    line-numbers))

(defn matching-close [text open-index open-char close-char]
  (loop [idx open-index
         depth 0
         in-string? false
         escape? false
         in-comment? false]
    (when (< idx (count text))
      (let [ch (.charAt text idx)]
        (cond
          in-comment?
          (recur (inc idx) depth in-string? false (not= ch \newline))

          escape? (recur (inc idx) depth in-string? false false)
          (and in-string? (= ch \\)) (recur (inc idx) depth in-string? true false)
          (= ch \") (recur (inc idx) depth (not in-string?) false false)
          in-string? (recur (inc idx) depth in-string? false false)
          (= ch \;) (recur (inc idx) depth false false true)
          (= ch open-char) (recur (inc idx) (inc depth) false false false)
          (= ch close-char) (let [next-depth (dec depth)]
                              (if (zero? next-depth)
                                idx
                                (recur (inc idx) next-depth false false false)))
          :else (recur (inc idx) depth false false false))))))

(defn enclosing-track-range [text caret]
  (loop [idx (previous-track-open text caret)]
    (when (>= idx 0)
      (if-let [end (matching-close text idx \( \))]
        (if (<= idx caret end)
          [idx end]
          (recur (previous-track-open text (dec idx))))
        (recur (previous-track-open text (dec idx)))))))

(defn find-fx-vector [text start end]
  (let [visible (code-visible-text text)
        fx-token (.indexOf visible ":fx" start)]
    (when (and (>= fx-token 0) (< fx-token end))
      (let [
            bracket (.indexOf text "[" fx-token)]
        (when (and (>= bracket 0) (<= bracket end))
          (when-let [close (matching-close text bracket \[ \])]
            (when (<= close end)
              [bracket close])))))))

(defn line-indent-before [text idx fallback]
  (let [bounded (max 0 (min idx (count text)))
        line-start (min bounded (inc (.lastIndexOf text "\n" (max 0 (dec bounded)))))
        line (bounded-subs text line-start bounded)
        spaces (apply str (take-while #(= % \space) line))]
    (if (seq spaces) spaces fallback)))

(defn track-property-indent [text start end]
  (let [track (subs text start (inc end))]
    (or (some->> (clojure.string/split-lines track)
                 rest
                 (some #(when-let [[_ spaces] (re-find #"^(\s+):" %)] spaces)))
        "   ")))

(defn fx-vector-form? [effect]
  (clojure.string/starts-with? (clojure.string/trim effect) ":fx"))

(defn fx-vector-body [effect]
  (let [trimmed (clojure.string/trim effect)
        open (.indexOf trimmed "[")]
    (if (>= open 0)
      (if-let [close (matching-close trimmed open \[ \])]
        (clojure.string/trim (subs trimmed (inc open) close))
        (subs trimmed (inc open)))
      trimmed)))

(defn normalize-effect-lines [effect indent]
  (let [body (if (fx-vector-form? effect)
               (fx-vector-body effect)
               (clojure.string/trim effect))]
    (->> (clojure.string/split-lines body)
         (map clojure.string/trim)
         (remove empty?)
         (map #(str indent %))
         (clojure.string/join "\n"))))

(defn effect-body [effect]
  (if (fx-vector-form? effect)
    (fx-vector-body effect)
    (clojure.string/trim effect)))

(defn effect-vector-replacement [existing effect property-indent]
  (let [effect-indent (str property-indent "  ")
        body (->> [(clojure.string/trim existing) (effect-body effect)]
                  (remove clojure.string/blank?)
                  (clojure.string/join "\n"))]
    (str "[\n"
         (normalize-effect-lines body effect-indent)
         "\n" property-indent "]")))

(defn effect-vector-insertion [effect property-indent]
  (let [effect-indent (str property-indent "  ")]
    (str "\n" property-indent ":fx [\n"
         (normalize-effect-lines effect effect-indent)
         "\n" property-indent "]")))

(defn track-fx-insertion-range [text start end]
  (let [insert-at (loop [idx (dec end)]
                    (cond
                      (<= idx start) end
                      (Character/isWhitespace (.charAt text idx)) (recur (dec idx))
                      :else (inc idx)))]
    [insert-at end]))

(defn insert-effect-smart! [^JTextComponent editor effect]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (if-let [[start end] (enclosing-track-range text caret)]
      (let [property-indent (track-property-indent text start end)]
        (if-let [[open close] (find-fx-vector text start end)]
          (let [existing (subs text (inc open) close)
                replacement (effect-vector-replacement existing effect property-indent)]
            (replace-text-range! editor replacement open (inc close))
            (.setCaretPosition editor (+ open (count replacement))))
          (let [insertion (effect-vector-insertion effect property-indent)
                [insert-start insert-end] (track-fx-insertion-range text start end)]
            (replace-text-range! editor insertion insert-start insert-end)
            (.setCaretPosition editor (+ insert-start (count insertion))))))
      (insert-at-caret! editor effect)))
  (.requestFocusInWindow editor))

(defn scene-name-from-field! [^JFrame frame ^JTextField scene-field]
  (let [typed (clojure.string/trim (.getText scene-field))
        name (if (clojure.string/blank? typed)
               (some-> (JOptionPane/showInputDialog frame "Scene name" "intro")
                       clojure.string/trim)
               typed)]
    (when-not (clojure.string/blank? name)
      (.setText scene-field name)
      name)))

(declare range-text indent-lines named-scene-range)

(defn form-symbol-at? [text idx symbol]
  (let [n (count text)
        open-ok? (and (< idx n) (= (.charAt text idx) \())
        after-open (inc idx)
        sym-end (+ after-open (count symbol))]
    (and open-ok?
         (<= sym-end n)
         (= symbol (subs text after-open sym-end))
         (or (= sym-end n)
             (Character/isWhitespace (.charAt text sym-end))
             (= (.charAt text sym-end) \))))))

(defn form-ranges [text symbol]
  (let [visible (code-visible-text text)]
    (loop [idx 0
           ranges []]
      (if-let [open (let [found (.indexOf visible "(" idx)]
                      (when (>= found 0) found))]
        (let [next-idx (inc open)]
          (if (form-symbol-at? visible open symbol)
            (if-let [close (matching-close text open \( \))]
              (recur (inc close) (conj ranges [open close]))
              (recur next-idx ranges))
            (recur next-idx ranges)))
        ranges))))

(defn named-scene-range [source scene]
  (let [visible (code-visible-text source)
        pattern (re-pattern (str "\\((?:scene|block)\\s+:" (java.util.regex.Pattern/quote scene) "\\b"))
        matcher (re-matcher pattern visible)]
    (when (.find matcher)
      (let [start (.start matcher)]
        (when-let [end (matching-close source start \( \))]
          [start end])))))

(defn scene-exists? [source scene]
  (boolean (named-scene-range source scene)))

(defn inside-any-range? [idx ranges]
  (some (fn [[start end]] (<= start idx end)) ranges))

(defn following-top-level-track-ranges [source scene-end]
  (let [scene-ranges (concat (form-ranges source "scene")
                             (form-ranges source "block"))]
    (->> (concat (form-ranges source "d")
                 (form-ranges source "sample"))
         (filter (fn [[start _]]
                   (and (> start scene-end)
                        (not (inside-any-range? start scene-ranges)))))
         (sort-by first)
         vec)))

(defn remove-ranges [source ranges]
  (reduce (fn [text [start end]]
            (str (subs text 0 start) (subs text (inc end))))
          source
          (sort-by first > ranges)))

(defn absorb-following-tracks-into-scene [source scene]
  (if-let [[_ scene-end] (named-scene-range source scene)]
    (let [track-ranges (following-top-level-track-ranges source scene-end)]
      (if (seq track-ranges)
        (let [tracks (->> track-ranges
                          (map #(range-text source [(first %) (inc (second %))]))
                          (map clojure.string/trim)
                          (clojure.string/join "\n\n"))
              insertion (str "\n" (indent-lines tracks "  "))
              with-tracks (str (subs source 0 scene-end)
                               insertion
                               (subs source scene-end))
              adjusted-ranges (map (fn [[start end]]
                                     [(+ start (count insertion))
                                      (+ end (count insertion))])
                                   track-ranges)]
          (clojure.string/trim (remove-ranges with-tracks adjusted-ranges)))
        source))
    source))

(defn selected-range [^JTextComponent editor]
  (let [start (.getSelectionStart editor)
        end (.getSelectionEnd editor)]
    (when (< start end)
      [start end])))

(defn range-text [text [start end]]
  (subs text start end))

(defn indent-lines [text indent]
  (->> (clojure.string/split-lines text)
       (map #(if (clojure.string/blank? %) % (str indent %)))
       (clojure.string/join "\n")))

(defn scene-wrapper
  ([scene body]
   (scene-wrapper scene body true))
  ([scene body include-comments?]
   (str "(scene :" scene " :repeat 1\n"
        (when include-comments?
          "  ; :loop true\n  ; :steps 16\n  ; :bars 1\n")
        (indent-lines (clojure.string/trim body) "  ")
        ")\n")))

(defn scene-block-range [^JTextComponent editor]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (or (selected-range editor)
        (when-let [[start end] (enclosing-track-range text caret)]
          [start (inc end)]))))

(defn ensure-scene-for-cue! [^JFrame frame ^JTextComponent editor scene]
  (let [source (.getText editor)]
    (if (scene-exists? source scene)
      (let [updated (absorb-following-tracks-into-scene source scene)]
        (when-not (= source updated)
          (.setText editor updated))
        updated)
      (if-let [[start end] (scene-block-range editor)]
        (let [block (range-text source [start end])
              replacement (scene-wrapper scene block)]
          (replace-text-range! editor replacement start end)
          (.setCaretPosition editor (+ start (count replacement)))
          (.getText editor))
        (do
          (JOptionPane/showMessageDialog frame
                                         "Select a block or place the cursor inside a track form first."
                                         "No block selected"
                                         JOptionPane/WARNING_MESSAGE)
          nil)))))
