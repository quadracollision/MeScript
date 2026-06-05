(ns glitchlisp-swing
  (:require [clojure.java.io :as io]
            [clojure.string :as str])
  (:import
    [java.awt BorderLayout Color Dimension Font]
    [java.awt.event ActionListener WindowAdapter]
    [java.io BufferedReader ByteArrayInputStream ByteArrayOutputStream File InputStreamReader OutputStreamWriter PushbackReader StringReader]
    [javax.sound.sampled AudioFileFormat$Type AudioFormat AudioInputStream AudioSystem Clip]
    [javax.swing.event CaretListener DocumentListener]
    [javax.swing.text DefaultHighlighter$DefaultHighlightPainter JTextComponent SimpleAttributeSet StyleConstants StyledDocument]
    [javax.swing AbstractAction BorderFactory Box BoxLayout JButton JCheckBoxMenuItem JComboBox JFileChooser JComponent JFrame JLabel JMenu JMenuBar JMenuItem JOptionPane JPanel JScrollPane JTextField JTextPane KeyStroke SwingUtilities Timer]))

(defn load-swing-module! [resource-path file-path]
  (when-not (.exists (java.io.File. file-path))
    nil)
  (cond
    (clojure.java.io/resource resource-path)
    (load-string (slurp (clojure.java.io/resource resource-path)))

    (.exists (java.io.File. file-path))
    (load-file file-path)

    :else
    (throw (java.io.FileNotFoundException. resource-path))))

(load-swing-module! "glitchlisp/swing/shared.clj" "src/glitchlisp/swing/shared.clj")
(load-swing-module! "glitchlisp/swing/catalog.clj" "src/glitchlisp/swing/catalog.clj")
(load-swing-module! "glitchlisp/swing/editor.clj" "src/glitchlisp/swing/editor.clj")
(load-swing-module! "glitchlisp/swing/render.clj" "src/glitchlisp/swing/render.clj")
(load-swing-module! "glitchlisp/swing/live.clj" "src/glitchlisp/swing/live.clj")

(def default-source glitchlisp.swing.catalog/default-source)
(def resource-slurp glitchlisp.swing.shared/resource-slurp)
(def file-or-resource-slurp glitchlisp.swing.shared/file-or-resource-slurp)
(def app-dir glitchlisp.swing.shared/app-dir)
(def child-file glitchlisp.swing.shared/child-file)
(def load-oscillators glitchlisp.swing.catalog/load-oscillators)
(def oscillator-sources glitchlisp.swing.catalog/oscillator-sources)
(def oscillator-option-labels glitchlisp.swing.catalog/oscillator-option-labels)
(def oscillator-option-source glitchlisp.swing.catalog/oscillator-option-source)
(def oscillator-option-header? glitchlisp.swing.catalog/oscillator-option-header?)
(def track-id-for-source glitchlisp.swing.catalog/track-id-for-source)
(def defaults-for-source glitchlisp.swing.catalog/defaults-for-source)
(def oscillator-parameter-examples glitchlisp.swing.catalog/oscillator-parameter-examples)
(def oscillator-structure-snippet glitchlisp.swing.catalog/oscillator-structure-snippet)
(def oscillator-snippet glitchlisp.swing.catalog/oscillator-snippet)
(def load-effects glitchlisp.swing.catalog/load-effects)
(def effect-options glitchlisp.swing.catalog/effect-options)
(def blank-effect-form glitchlisp.swing.catalog/blank-effect-form)
(def state glitchlisp.swing.shared/state)
(def set-status! glitchlisp.swing.shared/set-status!)
(def live-running? glitchlisp.swing.shared/live-running?)
(def live-status-lines glitchlisp.swing.shared/live-status-lines)
(def selected-oscillator-source glitchlisp.swing.catalog/selected-oscillator-source)
(def selected-effect-form glitchlisp.swing.catalog/selected-effect-form)
(def default-audio-device-label glitchlisp.swing.shared/default-audio-device-label)
(def selected-audio-device glitchlisp.swing.shared/selected-audio-device)
(def set-combo-items! glitchlisp.swing.shared/set-combo-items!)

(def insert-at-caret! glitchlisp.swing.editor/insert-at-caret!)
(def ensure-valid-caret! glitchlisp.swing.editor/ensure-valid-caret!)
(def replace-text-range! glitchlisp.swing.editor/replace-text-range!)
(def insert-text-at! glitchlisp.swing.editor/insert-text-at!)
(def leading-spaces glitchlisp.swing.editor/leading-spaces)
(def current-line-before-caret glitchlisp.swing.editor/current-line-before-caret)
(def delimiter-balance glitchlisp.swing.editor/delimiter-balance)
(def opening-delimiters glitchlisp.swing.editor/opening-delimiters)
(def closing-delimiters glitchlisp.swing.editor/closing-delimiters)
(def line-start-offset glitchlisp.swing.editor/line-start-offset)
(def line-indent-at-offset glitchlisp.swing.editor/line-indent-at-offset)
(def delimiter-stack glitchlisp.swing.editor/delimiter-stack)
(def form-head-near-open glitchlisp.swing.editor/form-head-near-open)
(def first-token-column-after glitchlisp.swing.editor/first-token-column-after)
(def vector-content-indent glitchlisp.swing.editor/vector-content-indent)
(def open-vector-before-caret glitchlisp.swing.editor/open-vector-before-caret)
(def vector-enter-indent glitchlisp.swing.editor/vector-enter-indent)
(def smart-next-line-indent glitchlisp.swing.editor/smart-next-line-indent)
(def line-end-offset glitchlisp.swing.editor/line-end-offset)
(def previous-track-indent glitchlisp.swing.editor/previous-track-indent)
(def replace-line-indent glitchlisp.swing.editor/replace-line-indent)
(def reindent-block-text glitchlisp.swing.editor/reindent-block-text)
(def align-current-line-to-vector glitchlisp.swing.editor/align-current-line-to-vector)
(def align-current-track-to-previous glitchlisp.swing.editor/align-current-track-to-previous)
(def install-auto-indent! glitchlisp.swing.editor/install-auto-indent!)
(def line-column-for-offset glitchlisp.swing.editor/line-column-for-offset)
(def offset-for-line-column glitchlisp.swing.editor/offset-for-line-column)
(def line-column-from-message glitchlisp.swing.editor/line-column-from-message)
(def clean-error-message glitchlisp.swing.editor/clean-error-message)
(def syntax-exception glitchlisp.swing.editor/syntax-exception)
(def validate-delimiters! glitchlisp.swing.editor/validate-delimiters!)
(def error-highlight-painter glitchlisp.swing.editor/error-highlight-painter)
(def paren-match-painter glitchlisp.swing.editor/paren-match-painter)
(def paren-peer-painter glitchlisp.swing.editor/paren-peer-painter)
(def paren-unmatched-painter glitchlisp.swing.editor/paren-unmatched-painter)
(def error-highlight-key glitchlisp.swing.editor/error-highlight-key)
(def paren-highlight-key glitchlisp.swing.editor/paren-highlight-key)
(def live-step-highlight-key glitchlisp.swing.editor/live-step-highlight-key)
(def live-gate-ranges-key glitchlisp.swing.editor/live-gate-ranges-key)
(def live-gate-ranges-text-key glitchlisp.swing.editor/live-gate-ranges-text-key)
(def syntax-refreshing-key glitchlisp.swing.editor/syntax-refreshing-key)
(def paren-refresh-timer-key glitchlisp.swing.editor/paren-refresh-timer-key)
(def live-highlight-delay-ms glitchlisp.swing.editor/live-highlight-delay-ms)
(def syntax-refresh-delay-ms glitchlisp.swing.editor/syntax-refresh-delay-ms)
(def paren-refresh-delay-ms glitchlisp.swing.editor/paren-refresh-delay-ms)
(def syntax-max-highlight-chars glitchlisp.swing.editor/syntax-max-highlight-chars)
(def syntax-form-names glitchlisp.swing.editor/syntax-form-names)
(def syntax-attrs glitchlisp.swing.editor/syntax-attrs)
(def syntax-default-attrs glitchlisp.swing.editor/syntax-default-attrs)
(def syntax-comment-attrs glitchlisp.swing.editor/syntax-comment-attrs)
(def syntax-string-attrs glitchlisp.swing.editor/syntax-string-attrs)
(def syntax-form-attrs glitchlisp.swing.editor/syntax-form-attrs)
(def syntax-keyword-attrs glitchlisp.swing.editor/syntax-keyword-attrs)
(def syntax-number-attrs glitchlisp.swing.editor/syntax-number-attrs)
(def syntax-note-attrs glitchlisp.swing.editor/syntax-note-attrs)
(def syntax-span-limit glitchlisp.swing.editor/syntax-span-limit)
(def token-delimiter? glitchlisp.swing.editor/token-delimiter?)
(def syntax-kind glitchlisp.swing.editor/syntax-kind)
(def syntax-spans glitchlisp.swing.editor/syntax-spans)
(def syntax-attrs-for-kind glitchlisp.swing.editor/syntax-attrs-for-kind)
(def syntax-highlight-enabled? glitchlisp.swing.editor/syntax-highlight-enabled?)
(def refresh-syntax-colors! glitchlisp.swing.editor/refresh-syntax-colors!)
(def install-syntax-highlighter! glitchlisp.swing.editor/install-syntax-highlighter!)
(def install-standard-edit-controls! glitchlisp.swing.editor/install-standard-edit-controls!)
(def clear-editor-undo-history! glitchlisp.swing.editor/clear-editor-undo-history!)
(def editor-undo-manager glitchlisp.swing.editor/editor-undo-manager)
(def run-editor-undo! glitchlisp.swing.editor/run-editor-undo!)
(def run-editor-redo! glitchlisp.swing.editor/run-editor-redo!)
(def editor-context-menu glitchlisp.swing.editor/editor-context-menu)
(def editor-undo-manager-key glitchlisp.swing.editor/editor-undo-manager-key)
(def editor-context-menu-key glitchlisp.swing.editor/editor-context-menu-key)
(def editor-pane glitchlisp.swing.editor/editor-pane)
(def clear-error-highlight! glitchlisp.swing.editor/clear-error-highlight!)
(def clear-paren-highlight! glitchlisp.swing.editor/clear-paren-highlight!)
(def clear-live-step-highlight! glitchlisp.swing.editor/clear-live-step-highlight!)
(def top-level-vector-cell-ranges glitchlisp.swing.editor/top-level-vector-cell-ranges)
(def gate-pattern-vector-ranges glitchlisp.swing.editor/gate-pattern-vector-ranges)
(def cached-gate-pattern-vector-ranges glitchlisp.swing.editor/cached-gate-pattern-vector-ranges)
(def clear-live-gate-range-cache! glitchlisp.swing.editor/clear-live-gate-range-cache!)
(def install-live-gate-range-cache! glitchlisp.swing.editor/install-live-gate-range-cache!)
(def highlight-live-step! glitchlisp.swing.editor/highlight-live-step!)
(def queue-live-step-highlight! glitchlisp.swing.editor/queue-live-step-highlight!)
(def highlight-editor-range! glitchlisp.swing.editor/highlight-editor-range!)
(def matching-open glitchlisp.swing.editor/matching-open)
(def delimiter-match-range glitchlisp.swing.editor/delimiter-match-range)
(def delimiter-offset-near-caret glitchlisp.swing.editor/delimiter-offset-near-caret)
(def refresh-paren-highlight! glitchlisp.swing.editor/refresh-paren-highlight!)
(def install-paren-highlighter! glitchlisp.swing.editor/install-paren-highlighter!)
(def error-offset glitchlisp.swing.editor/error-offset)
(def focus-source-error! glitchlisp.swing.editor/focus-source-error!)
(def text-line-count glitchlisp.swing.editor/text-line-count)
(def line-number-text glitchlisp.swing.editor/line-number-text)
(def refresh-line-numbers! glitchlisp.swing.editor/refresh-line-numbers!)
(def line-number-gutter glitchlisp.swing.editor/line-number-gutter)
(def matching-close glitchlisp.swing.editor/matching-close)
(def enclosing-track-range glitchlisp.swing.editor/enclosing-track-range)
(def find-fx-vector glitchlisp.swing.editor/find-fx-vector)
(def line-indent-before glitchlisp.swing.editor/line-indent-before)
(def track-property-indent glitchlisp.swing.editor/track-property-indent)
(def fx-vector-form? glitchlisp.swing.editor/fx-vector-form?)
(def fx-vector-body glitchlisp.swing.editor/fx-vector-body)
(def normalize-effect-lines glitchlisp.swing.editor/normalize-effect-lines)
(def effect-body glitchlisp.swing.editor/effect-body)
(def effect-vector-replacement glitchlisp.swing.editor/effect-vector-replacement)
(def effect-vector-insertion glitchlisp.swing.editor/effect-vector-insertion)
(def insert-effect-smart! glitchlisp.swing.editor/insert-effect-smart!)
(def scene-name-from-field! glitchlisp.swing.editor/scene-name-from-field!)
(def scene-exists? glitchlisp.swing.editor/scene-exists?)
(def form-symbol-at? glitchlisp.swing.editor/form-symbol-at?)
(def form-ranges glitchlisp.swing.editor/form-ranges)
(def named-scene-range glitchlisp.swing.editor/named-scene-range)
(def inside-any-range? glitchlisp.swing.editor/inside-any-range?)
(def following-top-level-track-ranges glitchlisp.swing.editor/following-top-level-track-ranges)
(def remove-ranges glitchlisp.swing.editor/remove-ranges)
(def absorb-following-tracks-into-scene glitchlisp.swing.editor/absorb-following-tracks-into-scene)
(def selected-range glitchlisp.swing.editor/selected-range)
(def range-text glitchlisp.swing.editor/range-text)
(def indent-lines glitchlisp.swing.editor/indent-lines)
(def scene-wrapper glitchlisp.swing.editor/scene-wrapper)
(def scene-block-range glitchlisp.swing.editor/scene-block-range)
(def ensure-scene-for-cue! glitchlisp.swing.editor/ensure-scene-for-cue!)
(def choose-file glitchlisp.swing.render/choose-file)
(def read-file glitchlisp.swing.render/read-file)
(def write-file! glitchlisp.swing.render/write-file!)
(def compile-glitchlisp-source glitchlisp.swing.render/compile-glitchlisp-source)
(def current-file-or-session! glitchlisp.swing.render/current-file-or-session!)
(def stop-clip! glitchlisp.swing.render/stop-clip!)
(def play-wav! glitchlisp.swing.render/play-wav!)
(def run-command! glitchlisp.swing.render/run-command!)
(def refresh-audio-devices! glitchlisp.swing.render/refresh-audio-devices!)
(def audio-devices! glitchlisp.swing.render/audio-devices!)
(def renderer-path glitchlisp.swing.render/renderer-path)
(def expected-renderer-capabilities glitchlisp.swing.render/expected-renderer-capabilities)
(def parse-renderer-capabilities glitchlisp.swing.render/parse-renderer-capabilities)
(def renderer-compatible? glitchlisp.swing.render/renderer-compatible?)
(def ensure-renderer! glitchlisp.swing.render/ensure-renderer!)
(def strip-playback-commands glitchlisp.swing.render/strip-playback-commands)
(def source-with-cue glitchlisp.swing.render/source-with-cue)
(def has-play-command? glitchlisp.swing.render/has-play-command?)
(def has-track-form? glitchlisp.swing.render/has-track-form?)
(def first-scene-name glitchlisp.swing.render/first-scene-name)
(def preview-source glitchlisp.swing.render/preview-source)
(def require-playback-form! glitchlisp.swing.render/require-playback-form!)
(def wav-file-for-name glitchlisp.swing.render/wav-file-for-name)
(def bpm-from-source glitchlisp.swing.render/bpm-from-source)
(def seconds-for-steps glitchlisp.swing.render/seconds-for-steps)
(def read-source-forms glitchlisp.swing.render/read-source-forms)
(def form-head glitchlisp.swing.render/form-head)
(def pair-value glitchlisp.swing.render/pair-value)
(def positive-int-value glitchlisp.swing.render/positive-int-value)
(def gcd-int glitchlisp.swing.render/gcd-int)
(def lcm-int glitchlisp.swing.render/lcm-int)
(def truthy-gate? glitchlisp.swing.render/truthy-gate?)
(def expand-gate-cell glitchlisp.swing.render/expand-gate-cell)
(def gate-step-bools glitchlisp.swing.render/gate-step-bools)
(def euclid-bools glitchlisp.swing.render/euclid-bools)
(def gate-summary-from-steps glitchlisp.swing.render/gate-summary-from-steps)
(def gate-pattern-summary glitchlisp.swing.render/gate-pattern-summary)
(def note-pattern-summary glitchlisp.swing.render/note-pattern-summary)
(def track-loop-steps glitchlisp.swing.render/track-loop-steps)
(def top-level-track? glitchlisp.swing.render/top-level-track?)
(def scene-form? glitchlisp.swing.render/scene-form?)
(def scene-name glitchlisp.swing.render/scene-name)
(def scene-option-value glitchlisp.swing.render/scene-option-value)
(def scene-body-forms glitchlisp.swing.render/scene-body-forms)
(def track-id glitchlisp.swing.render/track-id)
(def scene-inferred-steps glitchlisp.swing.render/scene-inferred-steps)
(def scene-steps-from-form glitchlisp.swing.render/scene-steps-from-form)
(def played-scene glitchlisp.swing.render/played-scene)
(def inferred-loop-steps glitchlisp.swing.render/inferred-loop-steps)
(def emit-form glitchlisp.swing.render/emit-form)
(def split-scene-options-and-body glitchlisp.swing.render/split-scene-options-and-body)
(def looped-scene-form glitchlisp.swing.render/looped-scene-form)
(def loop-render-source glitchlisp.swing.render/loop-render-source)
(def scene-repeat-from-form glitchlisp.swing.render/scene-repeat-from-form)
(def scene-total-steps-from-form glitchlisp.swing.render/scene-total-steps-from-form)
(def loop-preview-cycles glitchlisp.swing.render/loop-preview-cycles)
(def loop-boundary-fade-ms glitchlisp.swing.render/loop-boundary-fade-ms)
(def read-all-bytes glitchlisp.swing.render/read-all-bytes)
(def int16-le glitchlisp.swing.render/int16-le)
(def write-int16-le! glitchlisp.swing.render/write-int16-le!)
(def fade-gain glitchlisp.swing.render/fade-gain)
(def apply-loop-boundary-fade glitchlisp.swing.render/apply-loop-boundary-fade)
(def extract-loop-cycle-wav! glitchlisp.swing.render/extract-loop-cycle-wav!)
(def render-audio! glitchlisp.swing.render/render-audio!)
(def render-and-play! glitchlisp.swing.render/render-and-play!)
(def live-end-marker glitchlisp.swing.live/live-end-marker)
(def live-process-running? glitchlisp.swing.live/live-process-running?)
(def close-live-process! glitchlisp.swing.live/close-live-process!)
(def live-update-timeout-ms glitchlisp.swing.live/live-update-timeout-ms)
(def begin-live-update! glitchlisp.swing.live/begin-live-update!)
(def complete-live-update! glitchlisp.swing.live/complete-live-update!)
(def expire-live-update! glitchlisp.swing.live/expire-live-update!)
(def handle-live-line! glitchlisp.swing.live/handle-live-line!)
(def start-live-reader! glitchlisp.swing.live/start-live-reader!)
(def wait-live-ready! glitchlisp.swing.live/wait-live-ready!)

(defn ensure-live-process! [^JTextComponent editor ^JLabel status device]
  (glitchlisp.swing.live/ensure-live-process! editor status device ensure-renderer!))

(def send-live-command! glitchlisp.swing.live/send-live-command!)

(defn live-update! [^JFrame frame ^JTextComponent editor ^JLabel status device]
  (glitchlisp.swing.live/live-update!
    frame editor status device ensure-renderer! preview-source require-playback-form! compile-glitchlisp-source))

(def live-stop! glitchlisp.swing.live/live-stop!)

(defn save-current! [^JFrame frame ^JTextComponent editor ^JLabel status]
  (let [file (or (:file @state) (choose-file frame "Save"))]
    (when file
      (write-file! file (.getText editor))
      (swap! state assoc :file file)
      (set-status! status (str "saved " (.getPath file))))))

(defn menu-item [text f]
  (doto (JMenuItem. text)
    (.addActionListener (reify ActionListener
                          (actionPerformed [_ _] (f))))))

(defn checkbox-menu-item [text selected? f]
  (let [item (JCheckBoxMenuItem. text (boolean selected?))]
    (.addActionListener item
                        (reify ActionListener
                          (actionPerformed [_ _] (f (.isSelected item)))))
    item))

(def about-text
  "MeScript v0.2\nJune 4th 2026\nJacob Pereira\njacob.m.pereira@gmail.com")

(defn show-about! [^JFrame frame]
  (JOptionPane/showMessageDialog frame about-text "About MeScript" JOptionPane/INFORMATION_MESSAGE))

(defn choose-wav-file [parent]
  (let [chooser (JFileChooser. ".")]
    (.setSelectedFile chooser (File. "render.wav"))
    (when (= JFileChooser/APPROVE_OPTION (.showSaveDialog chooser parent))
      (.getSelectedFile chooser))))

(defn save-audio-to-file! [^JFrame frame ^JTextComponent editor ^JLabel status ^File file]
  (render-audio! frame editor status (.getPath file) (.getText editor) false false))

(defn save-audio! [^JFrame frame ^JTextComponent editor ^JLabel status]
  (when-let [file (choose-wav-file frame)]
    (save-audio-to-file! frame editor status file)))

(def post-fx-only-labels
  #{"reverse" "tape-stop" "granular" "granular-stretch" "spectral-freeze"
    "haas" "stereo-widen" "stereo-imager" "width-enhance" "freq-shift"
    "autopan" "ping-pong-delay"})

(defn live-fx-label? [label]
  (not (contains? post-fx-only-labels label)))

(defn post-fx-label? [label]
  (contains? post-fx-only-labels label))

(defn live-effect-options []
  (filter #(live-fx-label? (:label %)) effect-options))

(defn post-effect-options []
  (filter #(post-fx-label? (:label %)) effect-options))

(def insert-form-categories
  ["Oscillator" "FX" "Post FX" "Scene" "Math / Logic" "Pattern" "Playback"])

(defn insert-form-options [category]
  (case category
    "Oscillator" (oscillator-option-labels)
    ("FX" "Effect") (cons "on :gate" (map :label (live-effect-options)))
    "Post FX" (map :label (post-effect-options))
    "Scene" ["scene" "scene chain" "by-scene track"]
    "Math / Logic" ["+" "-" "*" "/"
                    "map and" "map or" "map not" "map transpose"
                    "range" "repeat" "rotate" "interleave"]
    "Pattern" ["p :repeat" "every-n" "euclid-rot" "held gates" "nested subdivisions"]
    "Playback" ["play-scene" "bpm" "mute" "solo" "clear"]
    []))

(defn set-insert-options! [^JComboBox form-combo category]
  (.removeAllItems form-combo)
  (doseq [option (insert-form-options category)]
    (.addItem form-combo option))
  (when (and (= category "Oscillator")
             (> (.getItemCount form-combo) 1)
             (oscillator-option-header? (.getItemAt form-combo 0)))
    (.setSelectedIndex form-combo 1)))

(defn insert-scene-template
  ([scene]
   (insert-scene-template scene true))
  ([scene _include-comments?]
   (str "(scene :" scene " :repeat 1\n"
        "  ; :steps 16\n"
        "  ; :bars 1\n"
        "  )\n")))

(defn effect-form-for-label
  ([label]
   (effect-form-for-label label true))
  ([label include-comments?]
   (cond
     (= label "FX Vector") ":fx []"
     (= label "on :gate") nil
     :else (blank-effect-form label include-comments?))))

(defn post-fx-form-for-label [label include-comments?]
  (str "(post-fx [\n  "
       (str/replace (effect-form-for-label label include-comments?) "\n" "\n  ")
       "\n])\n"))

(defn form-after-keyword [text start end keyword-text]
  (let [key-idx (.indexOf text keyword-text start)]
    (when (and (>= key-idx 0) (< key-idx end))
      (let [value-start (loop [idx (+ key-idx (count keyword-text))]
                          (cond
                            (>= idx end) nil
                            (Character/isWhitespace (.charAt text idx)) (recur (inc idx))
                            :else idx))]
        (when value-start
          (let [ch (.charAt text value-start)]
            (cond
              (= ch \() (when-let [close (matching-close text value-start \( \))]
                          (subs text value-start (inc close)))
              (= ch \[) (when-let [close (matching-close text value-start \[ \])]
                          (subs text value-start (inc close)))
              :else (let [value-end (loop [idx value-start]
                                      (if (or (>= idx end)
                                              (Character/isWhitespace (.charAt text idx))
                                              (= (.charAt text idx) \)))
                                        idx
                                        (recur (inc idx))))]
                      (subs text value-start value-end)))))))))

(defn current-track-gate-form [^JTextComponent editor]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (when-let [[start end] (enclosing-track-range text caret)]
      (form-after-keyword text start end ":gate"))))

(defn fx-gate-snippet [^JTextComponent editor]
  (str "(on :gate " (or (current-track-gate-form editor) "(p [])") "\n"
       "    )"))

(defn enclosing-list-range [text caret]
  (let [caret (max 0 (min caret (count text)))]
    (loop [idx (.lastIndexOf text "(" caret)]
      (when (>= idx 0)
        (if-let [end (matching-close text idx \( \))]
          (if (<= idx caret end)
            [idx end]
            (recur (.lastIndexOf text "(" (dec idx))))
          (recur (.lastIndexOf text "(" (dec idx))))))))

(defn list-head-at [text start]
  (when (and (< start (count text))
             (= (.charAt text start) \())
    (let [head-start (loop [idx (inc start)]
                       (cond
                         (>= idx (count text)) nil
                         (Character/isWhitespace (.charAt text idx)) (recur (inc idx))
                         :else idx))]
      (when head-start
        (let [head-end (loop [idx head-start]
                         (if (or (>= idx (count text))
                                 (Character/isWhitespace (.charAt text idx))
                                 (contains? #{\( \) \[ \]} (.charAt text idx)))
                           idx
                           (recur (inc idx))))]
          (subs text head-start head-end))))))

(def non-effect-wrapper-heads
  #{"p" "gate-hold" "by-scene"})

(defn enclosing-effect-range [text caret]
  (when-let [[track-start track-end] (enclosing-track-range text caret)]
    (when-let [[fx-open fx-close] (find-fx-vector text track-start track-end)]
      (when (<= fx-open caret fx-close)
        (loop [idx (.lastIndexOf text "(" caret)]
          (when (and (>= idx 0) (> idx fx-open))
            (let [previous (.lastIndexOf text "(" (dec idx))]
              (if-let [end (matching-close text idx \( \))]
                (if (and (<= idx caret end)
                         (< end fx-close)
                         (let [head (list-head-at text idx)]
                           (and (seq head)
                                (not (contains? non-effect-wrapper-heads head)))))
                  [idx end]
                  (recur previous))
                (recur previous)))))))))

(defn gate-wrapper-text [gate effect indent]
  (let [effect-indent (str indent "  ")
        effect-text (str/trim effect)]
    (str "(on :gate " gate
         (if (str/blank? effect-text)
           "\n"
           (str "\n" (indent-lines effect-text effect-indent) "\n"))
         indent ")")))

(defn wrap-effect-on-gate! [^JTextComponent editor]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        gate (or (current-track-gate-form editor) "(p [])")]
    (if-let [[start end] (enclosing-effect-range text caret)]
      (let [indent (line-indent-at-offset text start)
            effect (subs text start (inc end))
            replacement (gate-wrapper-text gate effect indent)]
        (replace-text-range! editor replacement start (inc end))
        (.setCaretPosition editor (+ start (count replacement))))
      (insert-effect-smart! editor (fx-gate-snippet editor)))))

(defn top-level-insert-offset [text caret]
  (if-let [[_ end] (loop [idx (.lastIndexOf text "(" (max 0 (min caret (count text))))
                          outermost nil]
                     (if (>= idx 0)
                       (if-let [end (matching-close text idx \( \))]
                         (recur (.lastIndexOf text "(" (dec idx))
                                (if (<= idx caret end)
                                  [idx end]
                                  outermost))
                         (recur (.lastIndexOf text "(" (dec idx)) outermost))
                       outermost))]
    (inc end)
    caret))

(defn ensure-leading-newline-for-top-level-insert [text offset snippet]
  (let [needs-leading-newline? (and (pos? offset)
                                    (not= (.charAt text (dec offset)) \newline))]
    (str (when needs-leading-newline? "\n")
         snippet)))

(defn insert-top-level-form! [^JTextComponent editor snippet]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        offset (top-level-insert-offset text caret)
        insertion (ensure-leading-newline-for-top-level-insert text offset snippet)]
    (.setCaretPosition editor offset)
    (insert-at-caret! editor insertion)))

(defn top-level-insert-category? [category]
  (#{"Scene" "Playback" "Post FX"} category))

(defn top-level-track-range-at-caret [text caret]
  (when-let [[start end] (enclosing-track-range text caret)]
    (let [scene-ranges (concat (form-ranges text "scene")
                               (form-ranges text "block"))]
      (when-not (inside-any-range? start scene-ranges)
        [start end]))))

(defn scene-insert-offset [text caret]
  (if-let [[start _] (top-level-track-range-at-caret text caret)]
    start
    (top-level-insert-offset text caret)))

(defn enclosing-scene-range [text caret]
  (let [caret (max 0 (min caret (count text)))
        ranges (concat (form-ranges text "scene")
                       (form-ranges text "block"))]
    (some (fn [[start end]]
            (when (<= start caret end)
              [start end]))
          (sort-by first > ranges))))

(defn scene-track-insert-offset [text caret]
  (when-let [[_ end] (enclosing-scene-range text caret)]
    end))

(defn scene-track-insertion [text offset snippet]
  (let [needs-leading-newline? (and (pos? offset)
                                    (not= (.charAt text (dec offset)) \newline))]
    (str (when needs-leading-newline? "\n")
         (indent-lines (str/trim snippet) "  ")
         "\n")))

(defn scene-name-from-snippet [snippet]
  (some-> (re-find #"\((?:scene|block)\s+:([^\s\)]+)" snippet)
          second))

(defn track-into-existing-scene [source scene [start end]]
  (let [track-text (clojure.string/trim (range-text source [start (inc end)]))
        without-track (remove-ranges source [[start end]])]
    (if-let [[_ scene-end] (named-scene-range without-track scene)]
      (let [insertion (str "\n" (indent-lines track-text "  "))]
        (str (subs without-track 0 scene-end)
             insertion
             (subs without-track scene-end)))
      source)))

(defn wrap-track-in-scene [source scene [start end] include-comments?]
  (let [track-text (range-text source [start (inc end)])
        replacement (scene-wrapper scene track-text include-comments?)]
    (str (subs source 0 start)
         replacement
         (subs source (inc end)))))

(defn insert-scene-around-track! [^JTextComponent editor snippet scene track-range include-comments?]
  (let [text (.getText editor)
        updated (if (scene-exists? text scene)
                  (track-into-existing-scene text scene track-range)
                  (wrap-track-in-scene text scene track-range include-comments?))]
    (.setText editor updated)
    (when-let [[_ end] (named-scene-range updated scene)]
      (.setCaretPosition editor (inc end)))))

(defn insert-scene-form! [^JTextComponent editor snippet include-comments?]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        scene (scene-name-from-snippet snippet)]
    (if-let [track-range (top-level-track-range-at-caret text caret)]
      (if scene
        (insert-scene-around-track! editor snippet scene track-range include-comments?)
        (let [offset (scene-insert-offset text caret)
              insertion (ensure-leading-newline-for-top-level-insert text offset snippet)]
          (.setCaretPosition editor offset)
          (insert-at-caret! editor insertion)))
      (let [offset (scene-insert-offset text caret)
            insertion (ensure-leading-newline-for-top-level-insert text offset snippet)]
        (.setCaretPosition editor offset)
        (insert-at-caret! editor insertion)))))

(defn track-form-insert-offset [text caret]
  (if-let [[_ end] (enclosing-track-range text caret)]
    (inc end)
    (or (scene-track-insert-offset text caret)
        (top-level-insert-offset text caret))))

(defn insert-track-form! [^JTextComponent editor snippet]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        offset (track-form-insert-offset text caret)
        insertion (if (and (not (enclosing-track-range text caret))
                           (scene-track-insert-offset text caret))
                    (scene-track-insertion text offset snippet)
                    (ensure-leading-newline-for-top-level-insert text offset snippet))]
    (.setCaretPosition editor offset)
    (insert-at-caret! editor insertion)))

(defn track-insert-category? [category]
  (#{"Oscillator"} category))

(defn scalar-token-char? [ch]
  (not (or (Character/isWhitespace ch)
           (contains? #{\( \) \[ \] \;} ch))))

(defn scalar-token-range-at-caret [text caret]
  (let [caret (max 0 (min caret (count text)))
        probe (cond
                (and (< caret (count text))
                     (scalar-token-char? (.charAt text caret))) caret
                (and (pos? caret)
                     (scalar-token-char? (.charAt text (dec caret)))) (dec caret)
                :else nil)]
    (when probe
      (let [start (loop [idx probe]
                    (if (and (pos? idx)
                             (scalar-token-char? (.charAt text (dec idx))))
                      (recur (dec idx))
                      idx))
            end (loop [idx (inc probe)]
                  (if (and (< idx (count text))
                           (scalar-token-char? (.charAt text idx)))
                    (recur (inc idx))
                    idx))]
        [start end]))))

(defn arithmetic-option? [option]
  (#{"+" "-" "*" "/"} option))

(defn arithmetic-token-wrapper [option token]
  (str "(" option " " token " )"))

(defn insert-math-logic-form! [^JTextComponent editor option snippet]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (if-let [[start end] (scalar-token-range-at-caret text caret)]
      (if (arithmetic-option? option)
        (replace-text-range! editor
                             (arithmetic-token-wrapper option (subs text start end))
                             start
                             end)
        (replace-text-range! editor snippet start end))
      (insert-at-caret! editor snippet))))

(defn insert-form-snippet
  ([^JTextComponent editor category option scene]
   (insert-form-snippet editor category option scene true))
  ([^JTextComponent editor category option scene include-comments?]
   (case category
     "Oscillator" (if-let [source (oscillator-option-source option)]
                    (oscillator-structure-snippet source include-comments?)
                    "")
     ("FX" "Effect") (if (= option "on :gate")
                (fx-gate-snippet editor)
                (effect-form-for-label option include-comments?))
     "Post FX" (post-fx-form-for-label option include-comments?)
     "Scene" (case option
               "scene" (insert-scene-template scene include-comments?)
               "scene chain" (str "(scene :" scene " :next :next-scene\n"
                                  "  )\n\n"
                                  "(scene :next-scene\n"
                                  "  )\n\n"
                                  "(play-scene :" scene ")\n")
               "by-scene track" (str "(d :lead\n"
                                     "   :src :sine-synth\n"
                                     "   :note (by-scene\n"
                                     "          :" scene " null\n"
                                     "          :else null)\n"
                                     "   :gate (by-scene\n"
                                     "          :" scene " null\n"
                                     "          :else null))\n")
               "")
     "Pattern" (case option
                 "p :repeat" "(p :repeat 2 [])"
                 "every-n" "(every-n 4 1 0)"
                 "euclid-rot" "(euclid-rot 5 16 0)"
                 "held gates" "(p [1_ 0 1_2 0])"
                 "nested subdivisions" "(p [[1 0] [0 1]])"
                 "")
     "Math / Logic" (case option
                      "+" "(+ )"
                      "-" "(- )"
                      "*" "(* )"
                      "/" "(/ )"
                      "map and" "(map and [] [])"
                      "map or" "(map or [] [])"
                      "map not" "(map not [])"
                      "map transpose" "(map transpose [] 12)"
                      "range" "(range )"
                      "repeat" "(repeat 2 [])"
                      "rotate" "(rotate 1 [])"
                      "interleave" "(interleave [] [])"
                      "")
     "Playback" (case option
                  "play-scene" (str "(play-scene :" scene ")\n")
                  "bpm" "(bpm )\n"
                  "mute" "(mute :track)\n"
                  "solo" "(solo :track)\n"
                  "clear" "(clear :track)\n"
                  "")
     "")))

(defn insert-selected-form! [^JTextComponent editor ^JComboBox category-combo ^JComboBox form-combo scene]
  (ensure-valid-caret! editor)
  (let [category (str (.getSelectedItem category-combo))
        option (str (.getSelectedItem form-combo))
        snippet (insert-form-snippet editor
                                     category
                                     option
                                     scene
                                     (not (:remove-insert-comments @state)))]
    (when-not (str/blank? snippet)
      (if (and (#{"FX" "Effect"} category)
               (= option "on :gate"))
        (wrap-effect-on-gate! editor)
        (if (#{"FX" "Effect"} category)
        (insert-effect-smart! editor snippet)
        (cond
          (= "Scene" category) (insert-scene-form! editor snippet (not (:remove-insert-comments @state)))
          (top-level-insert-category? category) (insert-top-level-form! editor snippet)
          (track-insert-category? category) (insert-track-form! editor snippet)
          (= "Math / Logic" category) (insert-math-logic-form! editor option snippet)
          :else (insert-at-caret! editor snippet)))))))

(defn build-ui [initial-file]
  (let [frame (JFrame. "temporaworkstation")
        editor (editor-pane)
        status (JLabel. "ready")
        insert-category-combo (JComboBox. (into-array String insert-form-categories))
        insert-form-combo (JComboBox.)
        tools (JPanel.)
        source (if initial-file (read-file initial-file) default-source)]
    (swap! state assoc :file initial-file)
    (.setText editor source)
    (clear-editor-undo-history! editor)
    (.setFont editor (Font. Font/MONOSPACED Font/PLAIN 13))
    (.setBackground editor Color/WHITE)
    (install-auto-indent! editor)
    (install-syntax-highlighter! editor)
    (install-paren-highlighter! editor)
    (install-live-gate-range-cache! editor)
    (.setLayout tools (BoxLayout. tools BoxLayout/Y_AXIS))
    (.setBorder tools (BorderFactory/createEmptyBorder 6 8 6 8))
    (.setPreferredSize tools (Dimension. 148 480))
    (.setMinimumSize tools (Dimension. 148 0))
    (.setMaximumSize tools (Dimension. 148 32767))
    (set-insert-options! insert-form-combo "Oscillator")

    (letfn [(button [text f]
              (doto (JButton. text)
                (.setName (str "mescript-" (str/lower-case text) "-button"))
                (.addActionListener (reify ActionListener
                                      (actionPerformed [_ _] (f))))))
            (compact! [component height]
              (.setMaximumSize component (Dimension. 132 height))
              (.setPreferredSize component (Dimension. 132 height))
              (.setMinimumSize component (Dimension. 132 height))
              (.setAlignmentX component java.awt.Component/LEFT_ALIGNMENT)
              component)
            (add-control! [component height]
              (.add tools (compact! component height))
              (.add tools (Box/createVerticalStrut 4)))
            (add-label! [text]
              (let [label (JLabel. text)]
                (.setAlignmentX label java.awt.Component/LEFT_ALIGNMENT)
                (.add tools label))
              (.add tools (Box/createVerticalStrut 3)))]
      (.addActionListener insert-category-combo
                          (reify ActionListener
                            (actionPerformed [_ _]
                              (set-insert-options! insert-form-combo
                                                   (str (.getSelectedItem insert-category-combo))))))
      (add-label! "Playback")
      (add-control! (button "Play"
                            #(live-update! frame editor status (:audio-device @state)))
                    27)
      (add-control! (button "Stop"
                            #(live-stop! editor status))
                    27)

      (add-label! "Insert Form")
      (add-control! insert-category-combo 25)
      (add-control! insert-form-combo 25)
      (add-control! (button "Insert"
                            #(insert-selected-form! editor
                                                    insert-category-combo
                                                    insert-form-combo
                                                    (or (first-scene-name (.getText editor)) "intro")))
                    27)
      (let [menu-bar (JMenuBar.)
            file-menu (JMenu. "File")
            preferences-menu (JMenu. "Preferences")
            about-menu (JMenu. "About")
            audio-menu (JMenu. "Audio")
            devices-menu (JMenu. "Output Device")]
        (letfn [(set-device-items! [devices]
                  (.removeAll devices-menu)
                  (.add devices-menu
                        (menu-item default-audio-device-label
                                   #(do
                                      (swap! state assoc :audio-device nil)
                                      (set-status! status "audio device: default"))))
                  (doseq [device devices]
                    (.add devices-menu
                          (menu-item device
                                     #(do
                                        (swap! state assoc :audio-device device)
                                        (set-status! status (str "audio device: " device)))))))
                (refresh-devices! []
                  (future
                    (try
                      (let [devices (audio-devices! status)]
                        (SwingUtilities/invokeLater
                          #(do
                             (set-device-items! devices)
                             (set-status! status (str "audio devices: " (count devices))))))
                      (catch Exception ex
                        (SwingUtilities/invokeLater
                          #(set-status! status (str "audio device refresh failed: " (.getMessage ex))))))))]
          (set-device-items! [])
          (.add file-menu
                (menu-item "Open..."
                           #(when-let [file (choose-file frame "Open")]
                              (.setText editor (read-file file))
                              (clear-editor-undo-history! editor)
                              (swap! state assoc :file file)
                              (set-status! status (str "opened " (.getPath file))))))
          (.add file-menu (menu-item "Save" #(save-current! frame editor status)))
          (.add file-menu (menu-item "Save Audio..." #(save-audio! frame editor status)))
          (.add preferences-menu
                (checkbox-menu-item "Remove Insert Comments"
                                    (:remove-insert-comments @state)
                                    #(do
                                       (swap! state assoc :remove-insert-comments %)
                                       (set-status! status
                                                    (if %
                                                      "insert comments: removed"
                                                      "insert comments: shown")))))
          (.add audio-menu devices-menu)
          (.add audio-menu (menu-item "Refresh Devices" #(refresh-devices!)))
          (.add about-menu (menu-item "About MeScript" #(show-about! frame)))
          (.add menu-bar file-menu)
          (.add menu-bar preferences-menu)
          (.add menu-bar audio-menu)
          (.add menu-bar about-menu)
          (.setJMenuBar frame menu-bar)
          (refresh-devices!)))

      (let [editor-scroll (JScrollPane. editor)]
        (.setBackground (.getViewport editor-scroll) Color/WHITE)
        (.setRowHeaderView editor-scroll (line-number-gutter editor))
        (.setPreferredSize editor-scroll (Dimension. 712 520))
        (.add (.getContentPane frame) editor-scroll BorderLayout/CENTER)
        (.add (.getContentPane frame) tools BorderLayout/EAST)
        (.add (.getContentPane frame) status BorderLayout/SOUTH)))

    (.setDefaultCloseOperation frame JFrame/EXIT_ON_CLOSE)
    (.setName frame "mescript-workstation-frame")
    (.setName editor "mescript-editor")
    (.setName status "mescript-status")
    (.putClientProperty (.getRootPane frame) "mescript.editor" editor)
    (.putClientProperty (.getRootPane frame) "mescript.status" status)
    (.addWindowListener frame
                        (proxy [WindowAdapter] []
                          (windowClosing [_]
                            (close-live-process!))))
    (.pack frame)
    (.setLocationRelativeTo frame nil)
    (.setVisible frame true)
    frame))

(defn -main [& args]
  (let [file (some-> (first args) File.)]
    (SwingUtilities/invokeLater #(build-ui file))))

(when-not (or (System/getenv "GLITCHLISP_NO_GUI")
              (System/getProperty "glitchlisp.noGui"))
  (apply -main *command-line-args*))
