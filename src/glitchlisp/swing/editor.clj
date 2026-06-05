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
  (.replaceSelection editor text)
  (.requestFocusInWindow editor))

(defn replace-text-range! [^JTextComponent editor text start end]
  (let [doc (.getDocument editor)]
    (.remove doc start (- end start))
    (.insertString doc start text nil)))

(defn insert-text-at! [^JTextComponent editor text offset]
  (.insertString (.getDocument editor) offset text nil))

(defn leading-spaces [text]
  (apply str (take-while #(= % \space) text)))

(defn current-line-before-caret [^JTextComponent editor]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        line-start (inc (.lastIndexOf text "\n" (max 0 (dec caret))))]
    (subs text line-start caret)))

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
  (inc (.lastIndexOf text "\n" (max 0 (dec offset)))))

(defn line-indent-at-offset [text offset]
  (let [start (line-start-offset text offset)
        line (subs text start (min (count text) offset))]
    (leading-spaces line)))

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
  (when (= (.charAt text open-offset) \()
    (let [after-open (subs text (inc open-offset))
          trimmed (clojure.string/triml after-open)]
      (some-> (re-find #"^([^\s\)\]]+)" trimmed)
              second))))

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
        stack (delimiter-stack (subs text 0 caret))]
    (some (fn [{:keys [ch offset]}]
            (when (= ch \[)
              offset))
          (reverse stack))))

(defn vector-enter-indent [text caret line-start]
  (when-let [vector-open (open-vector-before-caret text caret)]
    (when (>= vector-open line-start)
      (vector-content-indent text vector-open))))

(defn smart-next-line-indent [text caret]
  (let [caret (max 0 (min caret (count text)))
        before (subs text 0 caret)
        stack (delimiter-stack before)
        line-start (line-start-offset text caret)
        current-line (subs text line-start caret)]
    (if (clojure.string/blank? current-line)
      ""
      (or (vector-enter-indent text caret line-start)
          (if-let [{:keys [ch offset]} (peek stack)]
            (let [base (line-indent-at-offset text offset)
                  head (form-head-near-open text offset)]
              (cond
                (= ch \[) (vector-content-indent text offset)
                (= head "d") (str base "   ")
                (contains? #{"scene" "block" "def" "tracks"} head) (str base "  ")
                :else (str base "  ")))
            (leading-spaces current-line))))))

(defn line-end-offset [text offset]
  (let [idx (.indexOf text "\n" (max 0 (min offset (count text))))]
    (if (neg? idx) (count text) idx)))

(declare matching-close enclosing-track-range)

(defn previous-track-indent [text track-start]
  (loop [idx (.lastIndexOf text "(d " (max 0 (dec track-start)))]
    (when (>= idx 0)
      (let [previous-idx (.lastIndexOf text "(d " (dec idx))]
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
              (.replaceSelection editor
                                 (str "\n"
                                      (smart-next-line-indent (.getText editor)
                                                              (.getCaretPosition editor)))))))
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
(def live-gate-ranges-key "glitchlisp.liveGateRanges")
(def live-gate-ranges-text-key "glitchlisp.liveGateRangesText")
(def syntax-refreshing-key "glitchlisp.syntaxRefreshing")
(def editor-undo-manager-key "glitchlisp.editorUndoManager")
(def editor-edit-controls-key "glitchlisp.editControlsInstalled")
(def editor-context-menu-key "glitchlisp.contextMenu")
(def live-highlight-delay-ms 33)
(def syntax-refresh-delay-ms 180)
(def paren-refresh-delay-ms 35)
(def syntax-max-highlight-chars 60000)

(def syntax-form-names
  #{"adsr" "and" "asdr" "block" "by-scene" "choose" "chord" "clear" "clear-all"
    "cue" "d" "def" "delay" "distort" "euclid" "euclid-rot" "every-n"
    "filter" "gate-hold" "gate-seq" "gate_seq" "gs" "interleave" "map"
    "master-fx" "mute" "not" "offset" "or" "p" "pan" "phaser" "play-block"
    "play-note" "play-scene" "post-fx" "range" "repeat" "rev" "reverb"
    "rotate" "s" "scale" "scene" "solo" "start!" "stop!" "take" "tracks" "transpose"
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
          (.putClientProperty editor syntax-refreshing-key false))))))

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

(defn paint-live-step-range! [^JTextComponent editor graphics [start end]]
  (when (< start end)
    (try
      (let [^Rectangle start-rect (.modelToView editor start)
            ^Rectangle end-rect (.modelToView editor (max start (dec end)))]
        (when (and start-rect end-rect)
          (let [x (.x start-rect)
                y (.y start-rect)
                width (max 2 (- (+ (.x end-rect) (.width end-rect)) x))
                height (.height start-rect)]
            (.setColor graphics live-step-fill-color)
            (.fillRect graphics x y width height)
            (.setColor graphics live-step-border-color)
            (.drawRect graphics x y (dec width) (dec height)))))
      (catch Exception _))))

(defn paint-live-step-overlay! [^JTextComponent editor graphics]
  (doseq [[start end] (or (.getClientProperty editor live-step-highlight-key) [])]
    (when (< start end)
      (paint-live-step-range! editor graphics [start end]))))

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
  (when (.getClientProperty editor live-step-highlight-key)
    (.putClientProperty editor live-step-highlight-key nil)
    (.repaint editor)))

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

(defn gate-pattern-vector-ranges [text]
  (loop [idx 0
         ranges []]
    (if-let [gate-idx (let [found (.indexOf text ":gate" idx)]
                        (when (>= found 0) found))]
      (let [p-idx (.indexOf text "(p" gate-idx)
            vector-open (when (>= p-idx 0) (.indexOf text "[" p-idx))]
        (if (and vector-open (>= vector-open 0))
          (if-let [vector-close (matching-close text vector-open \[ \])]
            (let [cells (top-level-vector-cell-ranges text vector-open vector-close)]
              (recur (inc vector-close) (conj ranges cells)))
            (recur (+ gate-idx 5) ranges))
          (recur (+ gate-idx 5) ranges)))
      ranges)))

(defn cached-gate-pattern-vector-ranges [^JTextComponent editor]
  (let [text (.getText editor)]
    (if (= text (.getClientProperty editor live-gate-ranges-text-key))
      (or (.getClientProperty editor live-gate-ranges-key) [])
      (let [ranges (gate-pattern-vector-ranges text)]
        (.putClientProperty editor live-gate-ranges-text-key text)
        (.putClientProperty editor live-gate-ranges-key ranges)
        ranges))))

(defn clear-live-gate-range-cache! [^JTextComponent editor]
  (.putClientProperty editor live-gate-ranges-text-key nil)
  (.putClientProperty editor live-gate-ranges-key nil))

(defn install-live-gate-range-cache! [^JTextComponent editor]
  (.addDocumentListener
    (.getDocument editor)
    (proxy [DocumentListener] []
      (insertUpdate [_] (clear-live-gate-range-cache! editor))
      (removeUpdate [_] (clear-live-gate-range-cache! editor))
      (changedUpdate [_] nil))))

(defn highlight-live-step! [^JTextComponent editor step]
  (let [ranges (vec
                 (keep (fn [cells]
                         (when (seq cells)
                           (let [[start end] (nth cells (mod step (count cells)))]
                             (when (< start end)
                               [start end]))))
                       (cached-gate-pattern-vector-ranges editor)))]
    (if (seq ranges)
      (do
        (.putClientProperty editor live-step-highlight-key ranges)
        (.repaint editor))
      (clear-live-step-highlight! editor))))

(defn queue-live-step-highlight! [^JTextComponent editor step]
  (let [should-schedule? (atom false)]
    (swap! shared/state
           (fn [current]
             (let [current (assoc current :live-highlight-step step)]
               (if (:live-highlight-scheduled current)
                 current
                 (do
                   (reset! should-schedule? true)
                   (assoc current :live-highlight-scheduled true))))))
    (when @should-schedule?
      (SwingUtilities/invokeLater
        #(let [timer (Timer. live-highlight-delay-ms nil)]
           (.setRepeats timer false)
           (.addActionListener
             timer
             (reify ActionListener
               (actionPerformed [_ _]
                 (let [step (:live-highlight-step @shared/state)]
                   (swap! shared/state assoc :live-highlight-scheduled false)
                   (when step
                     (highlight-live-step! editor step))))))
           (.start timer))))))

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

(defn error-offset [^JTextComponent editor ex]
  (let [data (ex-data ex)
        text (.getText editor)]
    (or (:offset data)
        (when (and (:line data) (:column data))
          (offset-for-line-column text (:line data) (:column data)))
        (when-let [{:keys [line column]} (line-column-from-message (.getMessage ex))]
          (offset-for-line-column text line column)))))

(defn focus-source-error! [^JTextComponent editor ^JLabel status ex]
  (when-let [offset (error-offset editor ex)]
    (let [text (.getText editor)
          start (max 0 (min offset (count text)))
          end (min (count text) (inc start))]
      (.requestFocusInWindow editor)
      (highlight-editor-range! editor start end)
      (set-status! status (clean-error-message ex)))))

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
  (loop [idx (.lastIndexOf text "(d " caret)]
    (when (>= idx 0)
      (if-let [end (matching-close text idx \( \))]
        (if (<= idx caret end)
          [idx end]
          (recur (.lastIndexOf text "(d " (dec idx))))
        (recur (.lastIndexOf text "(d " (dec idx)))))))

(defn find-fx-vector [text start end]
  (let [track (subs text start (inc end))
        rel (.indexOf track ":fx")]
    (when (>= rel 0)
      (let [fx-token (+ start rel)
            bracket (.indexOf text "[" fx-token)]
        (when (and (>= bracket 0) (<= bracket end))
          (when-let [close (matching-close text bracket \[ \])]
            (when (<= close end)
              [bracket close])))))))

(defn line-indent-before [text idx fallback]
  (let [line-start (inc (.lastIndexOf text "\n" (max 0 (dec idx))))
        line (subs text line-start idx)
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

(defn scene-exists? [source scene]
  (boolean (re-find (re-pattern (str "\\(scene\\s+:" (java.util.regex.Pattern/quote scene) "\\b")) source)))

(declare range-text indent-lines)

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
  (loop [idx 0
         ranges []]
    (if-let [open (let [found (.indexOf text "(" idx)]
                    (when (>= found 0) found))]
      (let [next-idx (inc open)]
        (if (form-symbol-at? text open symbol)
          (if-let [close (matching-close text open \( \))]
            (recur (inc close) (conj ranges [open close]))
            (recur next-idx ranges))
          (recur next-idx ranges)))
      ranges)))

(defn named-scene-range [source scene]
  (let [pattern (re-pattern (str "\\((?:scene|block)\\s+:" (java.util.regex.Pattern/quote scene) "\\b"))
        matcher (re-matcher pattern source)]
    (when (.find matcher)
      (let [start (.start matcher)]
        (when-let [end (matching-close source start \( \))]
          [start end])))))

(defn inside-any-range? [idx ranges]
  (some (fn [[start end]] (<= start idx end)) ranges))

(defn following-top-level-track-ranges [source scene-end]
  (let [scene-ranges (concat (form-ranges source "scene")
                             (form-ranges source "block"))]
    (->> (form-ranges source "d")
         (filter (fn [[start _]]
                   (and (> start scene-end)
                        (not (inside-any-range? start scene-ranges)))))
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
  ([scene body _include-comments?]
   (str "(scene :" scene " :repeat 1\n"
        "  ; :steps 16\n"
        "  ; :bars 1\n"
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
                                         "Select a block or place the cursor inside a (d ...) block first."
                                         "No block selected"
                                         JOptionPane/WARNING_MESSAGE)
          nil)))))
