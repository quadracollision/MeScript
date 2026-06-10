(ns glitchlisp-swing
  (:require [clojure.java.io :as io]
            [clojure.set :as set]
            [clojure.string :as str])
  (:import
    [java.awt BorderLayout Color Cursor Dimension Font GraphicsEnvironment]
    [java.awt.event ActionListener FocusAdapter InputEvent KeyEvent MouseAdapter MouseEvent WindowAdapter]
    [java.io BufferedReader ByteArrayInputStream ByteArrayOutputStream File InputStreamReader OutputStreamWriter PushbackReader StringReader]
    [javax.sound.sampled AudioFileFormat$Type AudioFormat AudioInputStream AudioSystem Clip]
    [javax.swing.event CaretListener ChangeListener DocumentListener]
    [javax.swing.text DefaultHighlighter$DefaultHighlightPainter JTextComponent SimpleAttributeSet StyleConstants StyledDocument]
    [javax.swing AbstractAction BorderFactory Box BoxLayout JButton JCheckBoxMenuItem JComboBox JFileChooser JComponent JFrame JLabel JMenu JMenuBar JMenuItem JOptionPane JPanel JScrollPane JTabbedPane JTextField JTextPane KeyStroke SwingUtilities Timer]))

(defn load-swing-module! [resource-path file-path]
  (when-not (.exists (java.io.File. file-path))
    nil)
  (cond
    (.exists (java.io.File. file-path))
    (load-file file-path)

    (clojure.java.io/resource resource-path)
    (load-string (slurp (clojure.java.io/resource resource-path)))

    :else
    (throw (java.io.FileNotFoundException. resource-path))))

(load-swing-module! "glitchlisp/swing/shared.clj" "src/glitchlisp/swing/shared.clj")
(load-swing-module! "glitchlisp/swing/catalog.clj" "src/glitchlisp/swing/catalog.clj")
(load-swing-module! "glitchlisp/swing/docs.clj" "src/glitchlisp/swing/docs.clj")
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
(def oscillator-param-contracts glitchlisp.swing.catalog/oscillator-param-contracts)
(def oscillator-structure-snippet glitchlisp.swing.catalog/oscillator-structure-snippet)
(def oscillator-snippet glitchlisp.swing.catalog/oscillator-snippet)
(def language-reference-text glitchlisp.swing.docs/language-reference-text)
(def show-language-reference! glitchlisp.swing.docs/show-language-reference!)
(def load-effects glitchlisp.swing.catalog/load-effects)
(def effect-options glitchlisp.swing.catalog/effect-options)
(def effect-option-for-label glitchlisp.swing.catalog/effect-option-for-label)
(def effect-param-contracts glitchlisp.swing.catalog/effect-param-contracts)
(def effect-type-contracts glitchlisp.swing.catalog/effect-type-contracts)
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
(def highlight-live-step-for-symbols! glitchlisp.swing.editor/highlight-live-step-for-symbols!)
(def queue-live-step-highlight! glitchlisp.swing.editor/queue-live-step-highlight!)
(def highlight-editor-range! glitchlisp.swing.editor/highlight-editor-range!)
(def matching-open glitchlisp.swing.editor/matching-open)
(def delimiter-match-range glitchlisp.swing.editor/delimiter-match-range)
(def delimiter-offset-near-caret glitchlisp.swing.editor/delimiter-offset-near-caret)
(def refresh-paren-highlight! glitchlisp.swing.editor/refresh-paren-highlight!)
(def install-paren-highlighter! glitchlisp.swing.editor/install-paren-highlighter!)
(def error-offset glitchlisp.swing.editor/error-offset)
(def focus-source-error! glitchlisp.swing.editor/focus-source-error!)
(def report-source-error! glitchlisp.swing.editor/report-source-error!)
(def text-line-count glitchlisp.swing.editor/text-line-count)
(def line-number-text glitchlisp.swing.editor/line-number-text)
(def refresh-line-numbers! glitchlisp.swing.editor/refresh-line-numbers!)
(def line-number-gutter glitchlisp.swing.editor/line-number-gutter)
(def matching-close glitchlisp.swing.editor/matching-close)
(def code-visible-text glitchlisp.swing.editor/code-visible-text)
(def visible-symbol-set glitchlisp.swing.editor/visible-symbol-set)
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
(def include-path-from-line glitchlisp.swing.render/include-path-from-line)
(def canonical-file glitchlisp.swing.render/canonical-file)
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
(def send-compiled-live-update! glitchlisp.swing.live/send-compiled-live-update!)

(declare tab-editor editor-file)

(defn tab-editors [^JTabbedPane tabs]
  (vec
    (keep #(tab-editor (.getComponentAt tabs %))
          (range (.getTabCount tabs)))))

(defn included-files
  ([source source-file]
   (included-files source source-file #{}))
  ([source source-file seen]
   (let [base (or (some-> ^File source-file .getParentFile)
                  (File. "."))
         direct (keep (fn [line]
                        (when-let [include-path (include-path-from-line line)]
                          (let [file (File. include-path)]
                            (canonical-file
                              (if (.isAbsolute file) file (File. base include-path))))))
                      (str/split-lines source))]
     (reduce
       (fn [result ^File file]
         (if (contains? result file)
           result
           (let [result (conj result file)
                 nested (try
                          (included-files (slurp (.getPath file)) file result)
                          (catch Exception _ result))]
             nested)))
       seen
       direct))))

(defn scene-symbols [source scene]
  (when scene
    (when-let [[start end] (glitchlisp.swing.editor/scene-form-range-by-id source scene)]
      (visible-symbol-set (code-visible-text (subs source start (inc end)))))))

(defn def-names-in-source [source]
  (set (map second (re-seq #"\(def\s+([^\s\(\)\[\];]+)"
                           (code-visible-text source)))))

(defn open-include-editors [^JTabbedPane tabs include-files]
  (vec
    (filter (fn [^JTextComponent editor]
              (when-let [file (editor-file editor)]
                (contains? include-files (canonical-file file))))
            (tab-editors tabs))))

(defn live-highlight-fn [^JTabbedPane tabs ^JTextComponent root-editor]
  (fn [step scene _received-ns]
    (SwingUtilities/invokeLater
      #(let [source (.getText root-editor)
             include-files (included-files source (or (editor-file root-editor)
                                                      (current-file-or-session!)))
             scene-symbols (scene-symbols source scene)
             include-editors (set (open-include-editors tabs include-files))]
         (doseq [editor (tab-editors tabs)]
           (cond
             (= editor root-editor)
             (highlight-live-step! editor step scene)

             (contains? include-editors editor)
             (let [symbols (when scene-symbols
                             (set/intersection scene-symbols
                                               (def-names-in-source (.getText editor))))]
               (if (or (nil? scene-symbols) (seq symbols))
                 (highlight-live-step-for-symbols! editor step nil symbols)
                 (clear-live-step-highlight! editor)))

             :else
             (clear-live-step-highlight! editor)))))))

(defn live-update! [^JFrame frame ^JTextComponent editor ^JLabel status device]
  (glitchlisp.swing.live/live-update!
    frame editor status device ensure-renderer! preview-source require-playback-form! compile-glitchlisp-source
    (some-> (.getRootPane frame)
            (.getClientProperty "mescript.editor-tabs")
            (live-highlight-fn editor))))

(def live-stop! glitchlisp.swing.live/live-stop!)

(def live-auto-update-delay-ms 450)
(def live-auto-update-timer-key "glitchlisp.liveAutoUpdateTimer")

(defn valid-live-compiled-source [source]
  (let [preview (preview-source source)]
    (require-playback-form! preview)
    (compile-glitchlisp-source preview)))

(defn next-live-auto-edit-token! []
  (:live-auto-edit-token
    (swap! state update :live-auto-edit-token
           #(inc (long (or % 0))))))

(defn live-auto-apply-source! [^JTextComponent editor ^JLabel status source token]
  (future
    (try
      (let [compiled (valid-live-compiled-source source)]
        (when (and (= token (:live-auto-edit-token @state))
                   (live-process-running?))
          (swap! state assoc :live-auto-last-error nil)
          (send-compiled-live-update! editor status compiled "live edit applied")))
      (catch Exception ex
        (when (= token (:live-auto-edit-token @state))
          (swap! state assoc :live-auto-last-error (clean-error-message ex))
          (SwingUtilities/invokeLater
            #(when (live-process-running?)
               (set-status! status "live edit pending: waiting for valid source"))))))))

(defn schedule-live-auto-update! [^JTextComponent editor ^JLabel status]
  (let [token (next-live-auto-edit-token!)]
    (when (live-process-running?)
      (when-let [^Timer timer (.getClientProperty editor live-auto-update-timer-key)]
        (.putClientProperty editor "glitchlisp.liveAutoUpdateToken" token)
        (.restart timer)))))

(defn install-live-auto-update! [^JTextComponent editor ^JLabel status]
  (when-not (.getClientProperty editor live-auto-update-timer-key)
    (let [timer (Timer. live-auto-update-delay-ms nil)]
      (.setRepeats timer false)
      (.addActionListener
        timer
        (reify ActionListener
          (actionPerformed [_ _]
            (let [token (or (.getClientProperty editor "glitchlisp.liveAutoUpdateToken")
                            (:live-auto-edit-token @state))
                  source (.getText editor)]
              (when (live-process-running?)
                (live-auto-apply-source! editor status source token))))))
      (.putClientProperty editor live-auto-update-timer-key timer)
      (.addDocumentListener
        (.getDocument editor)
        (reify DocumentListener
          (insertUpdate [_ _]
            (when-not (.getClientProperty editor syntax-refreshing-key)
              (schedule-live-auto-update! editor status)))
          (removeUpdate [_ _]
            (when-not (.getClientProperty editor syntax-refreshing-key)
              (schedule-live-auto-update! editor status)))
          (changedUpdate [_ _] nil))))))

(defn file-title [^File file untitled-index]
  (if file
    (.getName file)
    (str "Untitled " untitled-index)))

(defn editor-file [^JTextComponent editor]
  (.getClientProperty editor "mescript.file"))

(defn editor-tab-name [^JTextComponent editor untitled-index]
  (or (.getClientProperty editor "mescript.tab-name")
      (file-title (editor-file editor) untitled-index)))

(defn set-editor-tab-name! [^JTextComponent editor name]
  (.putClientProperty editor "mescript.tab-name" name))

(defn editor-dirty? [^JTextComponent editor]
  (boolean (.getClientProperty editor "mescript.dirty")))

(defn set-editor-dirty! [^JTextComponent editor dirty?]
  (.putClientProperty editor "mescript.dirty" (boolean dirty?)))

(defn set-editor-file! [^JTextComponent editor ^File file]
  (.putClientProperty editor "mescript.file" file)
  (swap! state assoc :file file))

(defn tab-editor [component]
  (cond
    (instance? JScrollPane component)
    (let [view (.getView (.getViewport ^JScrollPane component))]
      (when (instance? JTextComponent view)
        view))

    (instance? JTextComponent component)
    component

    :else nil))

(defn active-editor [^JTabbedPane tabs]
  (when tabs
    (tab-editor (.getSelectedComponent tabs))))

(defn active-file [^JTabbedPane tabs]
  (some-> (active-editor tabs) editor-file))

(defn sync-active-file! [^JTabbedPane tabs]
  (swap! state assoc :file (active-file tabs)))

(def tab-text-color (Color. 24 24 24))
(def tab-close-color (Color. 70 70 70))
(def tab-close-hover-color (Color. 180 40 40))

(defn refresh-tab-header-style! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (let [header (.getTabComponentAt tabs idx)]
          (when header
            (.setOpaque ^JComponent header false)
            (.setBorder ^JComponent header (BorderFactory/createEmptyBorder 2 5 2 4))
            (.revalidate ^JComponent header)
            (.repaint ^JComponent header)))
        (when-let [label (.getClientProperty editor "mescript.tab-label")]
          (.setForeground ^JLabel label tab-text-color))))))

(defn refresh-all-tab-headers! [^JTabbedPane tabs]
  (doseq [idx (range (.getTabCount tabs))]
    (when-let [editor (tab-editor (.getComponentAt tabs idx))]
      (refresh-tab-header-style! tabs editor)))
  (.revalidate tabs)
  (.repaint tabs))

(defn refresh-tab-title! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (let [title (str (when (editor-dirty? editor) "*")
                         (editor-tab-name editor (inc idx)))]
          (.setTitleAt tabs idx title)
          (when-let [label (.getClientProperty editor "mescript.tab-label")]
            (.setText ^JLabel label title))
          (refresh-tab-header-style! tabs editor))))))

(declare close-tab!)
(declare rename-tab!)

(defn select-editor-tab! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (.setSelectedIndex tabs idx)
        (sync-active-file! tabs)
        (refresh-all-tab-headers! tabs)
        true))))

(declare finish-inline-tab-rename!)

(defn start-inline-tab-rename! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (when-let [header (.getTabComponentAt tabs idx)]
          (when-let [label (.getClientProperty editor "mescript.tab-label")]
            (let [current (editor-tab-name editor (inc idx))
                  field (doto (JTextField. current)
                          (.setColumns (max 8 (count current)))
                          (.setBorder (BorderFactory/createEmptyBorder 0 3 0 3)))]
              (.putClientProperty editor "mescript.tab-rename-field" field)
              (.remove ^JPanel header ^JLabel label)
              (.add ^JPanel header field 0)
              (.revalidate ^JPanel header)
              (.repaint ^JPanel header)
              (.selectAll field)
              (.requestFocusInWindow field)
              (.addActionListener field
                                  (proxy [ActionListener] []
                                    (actionPerformed [_]
                                      (finish-inline-tab-rename! tabs editor true))))
              (.registerKeyboardAction field
                                       (proxy [ActionListener] []
                                         (actionPerformed [_]
                                           (finish-inline-tab-rename! tabs editor false)))
                                       (KeyStroke/getKeyStroke "ESCAPE")
                                       JComponent/WHEN_FOCUSED)
              (.addFocusListener field
                                 (proxy [FocusAdapter] []
                                   (focusLost [_]
                                     (finish-inline-tab-rename! tabs editor true))))
              true)))))))

(defn finish-inline-tab-rename! [^JTabbedPane tabs ^JTextComponent editor commit?]
  (when-let [field (.getClientProperty editor "mescript.tab-rename-field")]
    (when-let [scroll (.getClientProperty editor "mescript.scroll")]
      (let [idx (.indexOfComponent tabs scroll)]
        (when (>= idx 0)
          (when-let [header (.getTabComponentAt tabs idx)]
            (when-let [label (.getClientProperty editor "mescript.tab-label")]
              (.putClientProperty editor "mescript.tab-rename-field" nil)
              (when commit?
                (let [renamed (str/trim (.getText ^JTextField field))]
                  (when-not (str/blank? renamed)
                    (set-editor-tab-name! editor renamed))))
              (.remove ^JPanel header ^JTextField field)
              (.add ^JPanel header ^JLabel label 0)
              (refresh-tab-title! tabs editor)
              (.revalidate ^JPanel header)
              (.repaint ^JPanel header)
              (.repaint tabs)
              true)))))))

(defn install-tab-header! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (let [title-label (doto (JLabel. "")
                            (.setFocusable false)
                            (.setRequestFocusEnabled false)
                            (.setBorder (BorderFactory/createEmptyBorder 0 4 0 5)))
              close-label (doto (JLabel. "x")
                            (.setFont (Font. Font/SANS_SERIF Font/BOLD 11))
                            (.setForeground tab-close-color)
                            (.setFocusable false)
                            (.setRequestFocusEnabled false)
                            (.setBorder (BorderFactory/createEmptyBorder 0 3 0 3))
                            (.setCursor (Cursor/getPredefinedCursor Cursor/HAND_CURSOR))
                            (.setToolTipText "Close tab"))
              header (JPanel.)]
          (.setOpaque header false)
          (.setFocusable header false)
          (.setRequestFocusEnabled header false)
          (.setLayout header (BoxLayout. header BoxLayout/X_AXIS))
          (.putClientProperty editor "mescript.tab-label" title-label)
          (.addMouseListener header
                             (proxy [MouseAdapter] []
                               (mousePressed [^MouseEvent _]
                                 (select-editor-tab! tabs editor))))
          (.addMouseListener title-label
                             (proxy [MouseAdapter] []
                               (mousePressed [^MouseEvent _]
                                 (select-editor-tab! tabs editor))
                               (mouseClicked [^MouseEvent event]
                                 (when (= 2 (.getClickCount event))
                                   (start-inline-tab-rename! tabs editor)))))
          (.addMouseListener close-label
                             (proxy [MouseAdapter] []
                               (mouseClicked [^MouseEvent _]
                                 (close-tab!
                                   (.getClientProperty tabs "mescript.frame")
                                   tabs
                                   editor
                                   (.getClientProperty tabs "mescript.status")))
                               (mouseEntered [^MouseEvent _]
                                 (.setForeground close-label tab-close-hover-color))
                               (mouseExited [^MouseEvent _]
                                 (.setForeground close-label tab-close-color))))
          (.add header title-label)
          (.add header close-label)
          (.setTabComponentAt tabs idx header)
          (refresh-tab-title! tabs editor))))))

(defn normalize-save-name [name]
  (let [trimmed (str/trim (or name ""))]
    (when-not (str/blank? trimmed)
      (if (re-find #"\.[^/\\]+$" trimmed)
        trimmed
        (str trimmed ".gl")))))

(defn suggested-save-file [^JTextComponent editor]
  (when-let [name (normalize-save-name (editor-tab-name editor 1))]
    (File. name)))

(defn choose-file-for-editor [^JFrame frame title ^JTextComponent editor]
  (let [chooser (JFileChooser. ".")]
    (.setDialogTitle chooser title)
    (when-let [file (suggested-save-file editor)]
      (.setSelectedFile chooser file))
    (when (= JFileChooser/APPROVE_OPTION (.showSaveDialog chooser frame))
      (.getSelectedFile chooser))))

(defn rename-tab! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor]
  (start-inline-tab-rename! tabs editor))

(defn save-editor! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status]
  (let [file (or (editor-file editor) (choose-file-for-editor frame "Save" editor))]
    (when file
      (write-file! file (.getText editor))
      (set-editor-file! editor file)
      (set-editor-dirty! editor false)
      (refresh-tab-title! tabs editor)
      (set-status! status (str "saved " (.getPath file)))
      true)))

(defn prompt-save-tab! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status]
  (if-not (editor-dirty? editor)
    true
    (let [choice (JOptionPane/showConfirmDialog
                   frame
                   (str "Save changes to " (editor-tab-name editor 1) "?")
                   "Unsaved Changes"
                   JOptionPane/YES_NO_CANCEL_OPTION
                   JOptionPane/WARNING_MESSAGE)]
      (cond
        (= choice JOptionPane/YES_OPTION)
        (boolean (save-editor! frame tabs editor status))

        (= choice JOptionPane/NO_OPTION)
        true

        :else
        false))))

(defn configure-editor! [^JTextComponent editor ^JLabel status source file]
  (.setText editor source)
  (clear-editor-undo-history! editor)
  (.setFont editor (Font. Font/MONOSPACED Font/PLAIN 13))
  (.setBackground editor Color/WHITE)
  (.putClientProperty editor "mescript.file" file)
  (set-editor-dirty! editor false)
  (install-auto-indent! editor)
  (install-syntax-highlighter! editor)
  (install-paren-highlighter! editor)
  (install-live-gate-range-cache! editor)
  (install-live-auto-update! editor status)
  editor)

(defn add-editor-tab! [^JTabbedPane tabs ^JLabel status source file]
  (let [editor (configure-editor! (editor-pane) status source file)
        scroll (JScrollPane. editor)]
    (.setBackground (.getViewport scroll) Color/WHITE)
    (.setRowHeaderView scroll (line-number-gutter editor))
    (.setPreferredSize scroll (Dimension. 712 520))
    (.putClientProperty editor "mescript.scroll" scroll)
    (.setName editor "mescript-editor")
    (.addTab tabs (file-title file (inc (.getTabCount tabs))) scroll)
    (.addDocumentListener
      (.getDocument editor)
      (reify DocumentListener
        (insertUpdate [_ _]
          (set-editor-dirty! editor true)
          (refresh-tab-title! tabs editor))
        (removeUpdate [_ _]
          (set-editor-dirty! editor true)
          (refresh-tab-title! tabs editor))
        (changedUpdate [_ _] nil)))
    (.setSelectedComponent tabs scroll)
    (install-tab-header! tabs editor)
    (sync-active-file! tabs)
    editor))

(defn close-tab! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status]
  (when (or (nil? frame)
            (prompt-save-tab! frame tabs editor status))
    (when-let [scroll (.getClientProperty editor "mescript.scroll")]
      (let [idx (.indexOfComponent tabs scroll)]
        (when (>= idx 0)
          (.removeTabAt tabs idx)
          (when (zero? (.getTabCount tabs))
            (add-editor-tab! tabs (or status (JLabel.)) "" nil))
          (sync-active-file! tabs)
          (when status
            (set-status! status "closed tab"))
          true)))))

(defn close-all-tabs! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (loop []
    (if (zero? (.getTabCount tabs))
      true
      (let [component (.getComponentAt tabs 0)
            editor (tab-editor component)]
        (if (and editor (prompt-save-tab! frame tabs editor status))
          (do
            (.removeTabAt tabs 0)
            (recur))
          false)))))

(defn save-current! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when-let [editor (active-editor tabs)]
    (save-editor! frame tabs editor status)))

(defn save-as! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when-let [editor (active-editor tabs)]
    (when-let [file (choose-file-for-editor frame "Save As" editor)]
      (write-file! file (.getText editor))
      (set-editor-file! editor file)
      (set-editor-dirty! editor false)
      (refresh-tab-title! tabs editor)
      (set-status! status (str "saved " (.getPath file))))))

(defn bind-app-action! [^JComponent component ^KeyStroke keystroke action-key f]
  (.put (.getInputMap component JComponent/WHEN_IN_FOCUSED_WINDOW)
        keystroke
        action-key)
  (.put (.getActionMap component)
        action-key
        (proxy [AbstractAction] []
          (actionPerformed [_] (f)))))

(def save-current-keystroke
  (KeyStroke/getKeyStroke KeyEvent/VK_S InputEvent/ALT_DOWN_MASK))

(defn install-save-shortcut! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (bind-app-action! (.getRootPane frame)
                    save-current-keystroke
                    "mescript-save-current"
                    #(save-current! frame tabs status)))

(defn new-file! [^JTabbedPane tabs ^JLabel status]
  (add-editor-tab! tabs status "" nil)
  (set-status! status "new file"))

(defn open-file-in-tab! [^JTabbedPane tabs ^JLabel status ^File file]
  (if-let [existing (some (fn [idx]
                            (let [component (.getComponentAt tabs idx)
                                  editor (tab-editor component)
                                  open-file (some-> editor editor-file)]
                              (when (and open-file (= (.getCanonicalFile open-file)
                                                      (.getCanonicalFile file)))
                                component)))
                          (range (.getTabCount tabs)))]
    (do
      (.setSelectedComponent tabs existing)
      (sync-active-file! tabs)
      (set-status! status (str "opened " (.getPath file))))
    (do
      (add-editor-tab! tabs status (read-file file) file)
      (set-status! status (str "opened " (.getPath file))))))

(defn save-audio-to-file! [^JFrame frame ^JTextComponent editor ^JLabel status ^File file]
  (render-audio! frame editor status (.getPath file) (.getText editor) false false))

(declare choose-wav-file)

(defn save-audio! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when-let [editor (active-editor tabs)]
    (when-let [file (choose-wav-file frame)]
      (save-audio-to-file! frame editor status file))))

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
  "MeScript v0.35\nJune 10, 2026\nJacob Pereira\njacob.m.pereira@gmail.com")

(defn show-about! [^JFrame frame]
  (JOptionPane/showMessageDialog frame about-text "About MeScript" JOptionPane/INFORMATION_MESSAGE))

(defn choose-wav-file [parent]
  (let [chooser (JFileChooser. ".")]
    (.setSelectedFile chooser (File. "render.wav"))
    (when (= JFileChooser/APPROVE_OPTION (.showSaveDialog chooser parent))
      (.getSelectedFile chooser))))

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
  ["Oscillator" "FX" "Post FX" "Scene" "Playback"])

(defn insert-form-options [category]
  (case category
    "Oscillator" (oscillator-option-labels)
    ("FX" "Effect") (cons "on :gate" (map :label (live-effect-options)))
    "Post FX" (map :label (post-effect-options))
    "Scene" ["scene" "looping scene" "scene chain" "by-scene track"]
    "Math / Logic" ["+" "-" "*" "/"
                    "map and" "map or" "map not" "map transpose"
                    "range" "repeat" "take" "reverse" "rotate" "interleave"
                    "choose" "rand-range" "scale" "chord" "custom chord" "shape" "arpeggio" "transpose"]
    "Pattern" ["p :repeat" "then / times" "every-n" "euclid-rot" "held gates" "nested subdivisions"]
    "Playback" ["start!" "stop!" "play-scene" "play-note" "bpm" "mute" "unmute" "solo" "unsolo" "clear" "clear-all"]
    []))

(defn set-insert-options! [^JComboBox form-combo category]
  (.removeAllItems form-combo)
  (doseq [option (insert-form-options category)]
    (.addItem form-combo option)))

(defn insert-scene-template
  ([scene]
   (insert-scene-template scene true))
  ([scene include-comments?]
   (str "(scene :" scene " :repeat 4\n"
        (when include-comments?
          "  ; :loop true\n  ; :steps 16\n  ; :bars 1\n")
        "  )\n")))

(defn effect-form-for-label
  ([label]
   (effect-form-for-label label true))
  ([label include-comments?]
   (cond
     (= label "FX Vector") (or (:form (effect-option-for-label label))
                               ":fx [(filter :type :lowpass :cutoff 1200 :res 0.35)
     (delay :time 0.125 :feedback 0.32 :mix 0.22)]")
     (= label "on :gate") nil
     :else (blank-effect-form label include-comments?))))

(defn post-fx-form-for-label
  ([label include-comments?]
   (str "(post-fx [\n  "
        (str/replace (effect-form-for-label label include-comments?) "\n" "\n  ")
        "\n])\n")))

(defn standalone-post-fx-snippet [post-fx]
  (str "(d :post-fx-demo\n"
       "   :src :sine-synth\n"
       "   :note c3\n"
       "   :dur 0.12\n"
       "   :amp 0.2\n"
       "   :gate (p [1 0 1 0]))\n\n"
       post-fx
       "\n(start!)\n"))

(defn form-after-keyword [text start end keyword-text]
  (let [visible (code-visible-text text)
        key-idx (.indexOf visible keyword-text start)]
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

(defn current-track-id [^JTextComponent editor]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)]
    (or
      (when-let [[start end] (enclosing-track-range text caret)]
        (when-let [match (re-find #"^\((?:d|sample)\s+(:[^\s\)\]]+)"
                                   (subs text start (inc end)))]
          (second match)))
      (let [ids (try
                  (->> (read-source-forms text)
                       (tree-seq coll? seq)
                       (keep #(when (and (seq? %)
                                         (contains? #{'d 'sample} (first %))
                                         (keyword? (second %)))
                                (str (second %))))
                       distinct
                       vec)
                  (catch Exception _
                    []))]
        (when (= 1 (count ids))
          (first ids))))))

(defn fx-gate-snippet [^JTextComponent editor]
  (str "(on :gate " (or (current-track-gate-form editor) "(p [1 0])") "\n"
       "    (delay :time 0.125 :feedback 0.25 :mix 0.25))"))

(defn playback-track-snippet [^JTextComponent editor form]
  (let [text (.getText editor)]
    (if-let [id (current-track-id editor)]
      (str "(" form " " id ")\n")
      (if (str/blank? text)
        (case form
          "unmute" "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(mute :lead)\n(unmute :lead)\n"
          "unsolo" "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(solo :lead)\n(unsolo :lead)\n"
          "clear" "(d :lead :src :sine-synth :note c3 :gate 1)\n(d :keep :src :sine-synth :note e3 :gate 1)\n(start!)\n(clear :lead)\n"
          (str "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(" form " :lead)\n"))
        (str "; (" form " :track)\n")))))

(defn playback-scene-snippet [^JTextComponent editor scene]
  (let [text (.getText editor)
        scene-id (str ":" scene)]
    (cond
      (scene-exists? text scene)
      (str "(play-scene " scene-id ")\n")

      (str/blank? text)
      (str "(scene " scene-id " :loop true\n"
           "  (d :lead :src :sine-synth :note c3 :gate 1))\n"
           "(play-scene " scene-id ")\n")

      :else
      (str "; (play-scene " scene-id ")\n"))))

(defn has-top-level-track? [source]
  (try
    (boolean
      (some #(and (seq? %)
                  (contains? #{'d 'sample} (first %))
                  (keyword? (second %)))
            (read-source-forms source)))
    (catch Exception _
      false)))

(defn playback-start-snippet [^JTextComponent editor]
  (let [text (.getText editor)]
    (cond
      (has-top-level-track? text)
      "(start!)\n"

      (str/blank? text)
      "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n"

      :else
      "; (start!)\n")))

(defn playback-stop-snippet [^JTextComponent editor]
  (if (str/blank? (.getText editor))
    "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(stop!)\n"
    "(stop!)\n"))

(defn playback-clear-all-snippet [^JTextComponent editor]
  (if (str/blank? (.getText editor))
    "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n(clear-all)\n"
    "(clear-all)\n"))

(defn standalone-fx-track-snippet [effect]
  (str "(d :fx-demo\n"
       "   :src :sine-synth\n"
       "   :note c3\n"
       "   :dur 0.12\n"
       "   :amp 0.2\n"
       "   :fx [\n"
       (indent-lines (str/trim effect) "     ")
       "\n   ]\n"
       "   :gate (p [1 0 1 0]))\n\n"
       "(start!)\n"))

(defn standalone-pattern-track-snippet [gate]
  (str "(d :pattern-demo\n"
       "   :src :click\n"
       "   :note c3\n"
       "   :dur 0.05\n"
       "   :amp 0.4\n"
       "   :gate " gate ")\n\n"
       "(start!)\n"))

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
        (let [visible (code-visible-text text)]
          (loop [idx (.lastIndexOf visible "(" caret)]
            (when (and (>= idx 0) (> idx fx-open))
              (let [previous (.lastIndexOf visible "(" (dec idx))]
                (if-let [end (matching-close text idx \( \))]
                  (if (and (<= idx caret end)
                           (< end fx-close)
                           (let [head (list-head-at text idx)]
                             (and (seq head)
                                  (not (contains? non-effect-wrapper-heads head)))))
                    [idx end]
                    (recur previous))
                  (recur previous))))))))))

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
        gate (or (current-track-gate-form editor) "(p [1 0])")]
    (if-let [[start end] (enclosing-effect-range text caret)]
      (let [indent (line-indent-at-offset text start)
            effect (subs text start (inc end))
            replacement (gate-wrapper-text gate effect indent)]
        (replace-text-range! editor replacement start (inc end))
        (.setCaretPosition editor (+ start (count replacement))))
      (insert-effect-smart! editor (fx-gate-snippet editor)))))

(defn top-level-insert-offset [text caret]
  (let [visible (code-visible-text text)
        caret (max 0 (min caret (count text)))]
    (if-let [[_ end] (loop [idx (.lastIndexOf visible "(" caret)
                          outermost nil]
                     (if (>= idx 0)
                       (if-let [end (matching-close text idx \( \))]
                         (recur (.lastIndexOf visible "(" (dec idx))
                                (if (<= idx caret end)
                                  [idx end]
                                  outermost))
                         (recur (.lastIndexOf visible "(" (dec idx)) outermost))
                       outermost))]
      (inc end)
      caret)))

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

(defn enclosing-def-range [text caret]
  (let [caret (max 0 (min caret (count text)))]
    (some (fn [[start end]]
            (when (<= start caret end)
              [start end]))
          (sort-by first > (form-ranges text "def")))))

(defn enclosing-open-form-start [text caret symbols]
  (let [visible (code-visible-text text)
        caret (max 0 (min caret (count text)))]
    (loop [idx (.lastIndexOf visible "(" caret)]
      (when (>= idx 0)
        (if (and (some #(form-symbol-at? text idx %) symbols)
                 (not (matching-close text idx \( \))))
          idx
          (recur (.lastIndexOf visible "(" (dec idx))))))))

(defn enclosing-open-scene-start [text caret]
  (enclosing-open-form-start text caret ["scene" "block"]))

(defn enclosing-open-def-start [text caret]
  (enclosing-open-form-start text caret ["def"]))

(defn scene-track-insert-offset [text caret]
  (when-let [[_ end] (enclosing-scene-range text caret)]
    end))

(defn def-track-insert-offset [text caret]
  (when-let [[_ end] (enclosing-def-range text caret)]
    end))

(defn trailing-whitespace-range-before [text offset]
  (let [start (loop [idx offset]
                (if (and (pos? idx)
                         (Character/isWhitespace (.charAt text (dec idx))))
                  (recur (dec idx))
                  idx))]
    (when (< start offset)
      [start offset])))

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
        (def-track-insert-offset text caret)
        (top-level-insert-offset text caret))))

(defn insert-track-form! [^JTextComponent editor snippet]
  (let [text (.getText editor)
        caret (.getCaretPosition editor)
        in-track? (boolean (enclosing-track-range text caret))
        in-open-scene? (and (not in-track?) (boolean (enclosing-open-scene-start text caret)))
        in-open-def? (and (not in-track?) (not in-open-scene?) (boolean (enclosing-open-def-start text caret)))
        offset (if (or in-open-scene? in-open-def?)
                 caret
                 (track-form-insert-offset text caret))
        in-scene? (and (not in-track?) (not in-open-scene?) (boolean (scene-track-insert-offset text caret)))
        in-def? (and (not in-track?) (not in-open-def?) (not in-scene?) (boolean (def-track-insert-offset text caret)))
        [replace-start replace-end] (if (or in-scene? in-def?)
                                      (or (trailing-whitespace-range-before text offset)
                                          [offset offset])
                                      [offset offset])
        [replace-start replace-end] (if (or in-open-scene? in-open-def?)
                                      (or (trailing-whitespace-range-before text offset)
                                          [offset offset])
                                      [replace-start replace-end])
        insertion (cond
                    in-open-scene?
                    (str (scene-track-insertion text replace-start snippet) ")")

                    in-open-def?
                    (str (ensure-leading-newline-for-top-level-insert text replace-start snippet) ")")

                    in-scene?
                    (scene-track-insertion text replace-start snippet)

                    :else
                    (ensure-leading-newline-for-top-level-insert text replace-start snippet))]
    (if (< replace-start replace-end)
      (replace-text-range! editor insertion replace-start replace-end)
      (do
        (.setCaretPosition editor offset)
        (insert-at-caret! editor insertion)))))

(defn track-insert-category? [category]
  (#{"Oscillator"} category))

(defn scalar-token-char? [ch]
  (not (or (Character/isWhitespace ch)
           (contains? #{\( \) \[ \] \;} ch))))

(defn scalar-token-range-at-caret [text caret]
  (let [visible (code-visible-text text)
        caret (max 0 (min caret (count text)))
        probe (cond
                (and (< caret (count text))
                     (scalar-token-char? (.charAt visible caret))) caret
                (and (pos? caret)
                     (scalar-token-char? (.charAt visible (dec caret)))) (dec caret)
                :else nil)]
    (when probe
      (let [start (loop [idx probe]
                    (if (and (pos? idx)
                             (scalar-token-char? (.charAt visible (dec idx))))
                      (recur (dec idx))
                      idx))
            end (loop [idx (inc probe)]
                  (if (and (< idx (count text))
                           (scalar-token-char? (.charAt visible idx)))
                    (recur (inc idx))
                    idx))]
        [start end]))))

(defn arithmetic-option? [option]
  (#{"+" "-" "*" "/"} option))

(defn arithmetic-token-wrapper [option token]
  (let [fallback (case option
                   ("+" "-") "1"
                   ("*" "/") "2"
                   "1")]
    (str "(" option " " token " " fallback ")")))

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
               "looping scene" (str "(scene :" scene " :loop true\n"
                                    "  (d :lead\n"
                                    "     :src :sine-synth\n"
                                    "     :note c3\n"
                                    "     :dur 0.12\n"
                                    "     :amp 0.2\n"
                                    "     :gate (p [1 0 1 0])))\n\n"
                                    "(play-scene :" scene ")\n")
               "scene chain" (str "(scene :" scene " :repeat 1 :next :next-scene\n"
                                  "  (d :lead\n"
                                  "     :src :sine-synth\n"
                                  "     :note c3\n"
                                  "     :dur 0.12\n"
                                  "     :amp 0.2\n"
                                  "     :gate (p [1 0 1 0])))\n\n"
                                  "(scene :next-scene :loop true\n"
                                  "  (d :lead\n"
                                  "     :src :sine-synth\n"
                                  "     :note e3\n"
                                  "     :dur 0.12\n"
                                  "     :amp 0.2\n"
                                  "     :gate (p [1 1 0 1])))\n\n"
                                  "(play-scene :" scene ")\n")
               "by-scene track" (str "(def lead\n"
                                     "  (d :lead\n"
                                     "     :src :sine-synth\n"
                                     "     :note (by-scene\n"
                                     "            :" scene " c3\n"
                                     "            :next-scene e3\n"
                                     "            :else g3)\n"
                                     "     :dur 0.12\n"
                                     "     :amp 0.2\n"
                                     "     :gate (by-scene\n"
                                     "            :" scene " (p [1 0 1 0])\n"
                                     "            :next-scene (p [1 1 0 1])\n"
                                     "            :else (p [1 0 0 0]))))\n\n"
                                     "(scene :" scene " :repeat 1 :next :next-scene\n"
                                     "  lead)\n\n"
                                     "(scene :next-scene :loop true\n"
                                     "  lead)\n\n"
                                     "(play-scene :" scene ")\n")
               "")
     "Pattern" (case option
                 "p :repeat" "(p :repeat 2 [1 0])"
                 "then / times" "(p (then\n     (times 2 [1 0 0 0])\n     [1 1 1 1]))"
                 "every-n" "(every-n 4 1 0)"
                 "euclid-rot" "(euclid-rot 5 16 0)"
                 "held gates" "(p [1_ 0 1_2 0])"
                 "nested subdivisions" "(p [[1 0] [0 1]])"
                 "")
     "Math / Logic" (case option
                      "+" "(+ 1 2)"
                      "-" "(- 4 1)"
                      "*" "(* 2 4)"
                      "/" "(/ 8 2)"
                      "map and" "(map and [1 0 1] [1 1 0])"
                      "map or" "(map or [1 0 1] [0 1 0])"
                      "map not" "(map not [1 0 1])"
                      "map transpose" "(map transpose [c3 d3 e3] 12)"
                      "range" "(range 0 8 1)"
                      "repeat" "(repeat 2 [1 0])"
                      "take" "(take 8 [1 0 1 0 1 0 1 0 1 0])"
                      "reverse" "(reverse [c3 e3 g3])"
                      "rotate" "(rotate 1 [1 0 0])"
                      "interleave" "(interleave [1 1] [0 0])"
                      "choose" "(choose :count 8 :seed 1 [c3 e3 g3])"
                      "rand-range" "(rand-range :count 8 :seed 1 :min 0 :max 1)"
                      "scale" "(scale c3 :minor 8)"
                      "chord" "(chord c3 :minor7)"
                      "custom chord" "(chord c3 [0 3 7 10])"
                      "shape" "(shape (chord c3 :minor7) [2 4])"
                      "arpeggio" "(arpeggio c3 :minor7)"
                      "transpose" "(transpose c3 12)"
                      "")
     "Playback" (case option
                  "start!" (playback-start-snippet editor)
                  "stop!" (playback-stop-snippet editor)
                  "play-scene" (playback-scene-snippet editor scene)
                  "play-note" "(play-note c3)\n"
                  "bpm" "(bpm 100)\n"
                  "mute" (playback-track-snippet editor "mute")
                  "unmute" (playback-track-snippet editor "unmute")
                  "solo" (playback-track-snippet editor "solo")
                  "unsolo" (playback-track-snippet editor "unsolo")
                  "clear" (playback-track-snippet editor "clear")
                  "clear-all" (playback-clear-all-snippet editor)
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
        (if (enclosing-track-range (.getText editor) (.getCaretPosition editor))
          (wrap-effect-on-gate! editor)
          (insert-track-form! editor (standalone-fx-track-snippet snippet)))
        (if (#{"FX" "Effect"} category)
        (if (enclosing-track-range (.getText editor) (.getCaretPosition editor))
          (insert-effect-smart! editor snippet)
          (insert-track-form! editor (standalone-fx-track-snippet snippet)))
        (cond
          (= "Scene" category) (insert-scene-form! editor snippet (not (:remove-insert-comments @state)))
          (and (= "Post FX" category)
               (str/blank? (.getText editor))) (insert-top-level-form! editor (standalone-post-fx-snippet snippet))
          (and (= "Oscillator" category)
               (str/blank? (.getText editor))) (insert-top-level-form! editor (str snippet "\n(start!)\n"))
          (top-level-insert-category? category) (insert-top-level-form! editor snippet)
          (track-insert-category? category) (insert-track-form! editor snippet)
          (= "Math / Logic" category) (insert-math-logic-form! editor option snippet)
          (and (= "Pattern" category)
               (str/blank? (.getText editor))) (insert-top-level-form! editor (standalone-pattern-track-snippet snippet))
          :else (insert-at-caret! editor snippet)))))))

(def null-parameter-names
  (delay
    (->> (concat (keys oscillator-param-contracts)
                 [":sample-path" ":sample" ":sample-data"]
                 (keys effect-param-contracts)
                 (mapcat keys (vals effect-type-contracts)))
         distinct
         (sort-by count >)
         vec)))

(defn null-param-line? [line]
  (let [trimmed (str/trim line)]
    (boolean
      (some (fn [param]
              (re-matches
                (re-pattern (str (java.util.regex.Pattern/quote param)
                                 "\\s+(?:null|nil)(?:\\s*;.*)?"))
                trimmed))
            @null-parameter-names))))

(defn remove-inline-null-param-pairs [line]
  (reduce (fn [[text changed?] param]
            (let [pattern (re-pattern
                            (str "\\s+"
                                 (java.util.regex.Pattern/quote param)
                                 "\\s+(?:null|nil)(?=(?:\\s|[\\)\\]]|;|$))"))
                  updated (str/replace text pattern "")]
              [updated (or changed? (not= text updated))]))
          [line false]
          @null-parameter-names))

(defn clear-null-parameters-text [text]
  (let [lines (str/split text #"\n" -1)]
    (->> lines
         (keep (fn [line]
                 (when-not (null-param-line? line)
                   (let [[updated changed?] (remove-inline-null-param-pairs line)]
                     (if changed?
                       (str/replace updated #"\s+;.*$" "")
                       updated)))))
         (str/join "\n"))))

(defn set-editor-text-without-undo-event! [^JTextComponent editor text]
  (let [previous (.getClientProperty editor syntax-refreshing-key)
        doc (.getDocument editor)]
    (.putClientProperty editor syntax-refreshing-key true)
    (try
      (.remove doc 0 (.getLength doc))
      (.insertString doc 0 text nil)
      (finally
        (.putClientProperty editor syntax-refreshing-key previous)))))

(defn replace-editor-text-undoably! [^JTextComponent editor before after edit-name]
  (set-editor-text-without-undo-event! editor after)
  (when-let [manager (editor-undo-manager editor)]
    (.addEdit
      manager
      (proxy [javax.swing.undo.AbstractUndoableEdit] []
        (getPresentationName [] edit-name)
        (undo []
          (proxy-super undo)
          (set-editor-text-without-undo-event! editor before)
          (.setCaretPosition editor 0)
          (refresh-syntax-colors! editor))
        (redo []
          (proxy-super redo)
          (set-editor-text-without-undo-event! editor after)
          (.setCaretPosition editor 0)
          (refresh-syntax-colors! editor))))))

(defn clear-null-parameters! [^JTextComponent editor status]
  (let [before (.getText editor)
        after (clear-null-parameters-text before)]
    (if (= before after)
      (set-status! status "null parameters: none")
      (do
        (replace-editor-text-undoably! editor before after "Clear Null Parameters")
        (.setCaretPosition editor 0)
        (.requestFocusInWindow editor)
        (refresh-syntax-colors! editor)
        (set-status! status "null parameters: cleared")))))

(defn build-ui [initial-file]
  (let [frame (JFrame. "temporaworkstation")
        tabs (JTabbedPane.)
        status (JLabel. "ready")
        insert-category-combo (JComboBox. (into-array String insert-form-categories))
        insert-form-combo (JComboBox.)
        tools (JPanel.)
        source (if initial-file (read-file initial-file) default-source)]
    (.setName tabs "mescript-editor-tabs")
    (.setFocusable tabs false)
    (.setRequestFocusEnabled tabs false)
    (.setTabLayoutPolicy tabs JTabbedPane/SCROLL_TAB_LAYOUT)
    (.setFont tabs (Font. Font/SANS_SERIF Font/PLAIN 12))
    (.setBorder tabs (BorderFactory/createEmptyBorder 2 2 0 2))
    (.putClientProperty tabs "mescript.frame" frame)
    (.putClientProperty tabs "mescript.status" status)
    (add-editor-tab! tabs status source initial-file)
    (.addChangeListener tabs
                        (reify ChangeListener
                          (stateChanged [_ _]
                            (sync-active-file! tabs)
                            (refresh-all-tab-headers! tabs)
                            (SwingUtilities/invokeLater
                              #(do
                                 (refresh-all-tab-headers! tabs)
                                 (.repaint tabs)))
                            (when-let [editor (active-editor tabs)]
                              (.putClientProperty (.getRootPane frame) "mescript.editor" editor)))))
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
                            #(when-let [editor (active-editor tabs)]
                               (sync-active-file! tabs)
                               (live-update! frame editor status (:audio-device @state))))
                    27)
      (add-control! (button "Stop"
                            #(when-let [editor (active-editor tabs)]
                               (live-stop! editor status)))
                    27)

      (add-label! "Insert Form")
      (add-control! insert-category-combo 25)
      (add-control! insert-form-combo 25)
      (add-control! (button "Insert"
                            #(when-let [editor (active-editor tabs)]
                               (insert-selected-form! editor
                                                      insert-category-combo
                                                      insert-form-combo
                                                      (or (first-scene-name (.getText editor)) "intro"))))
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
                (menu-item "New"
                           #(new-file! tabs status)))
          (.add file-menu
                (menu-item "Open..."
                           #(when-let [file (choose-file frame "Open")]
                              (open-file-in-tab! tabs status file))))
          (.add file-menu
                (doto (menu-item "Save" #(save-current! frame tabs status))
                  (.setAccelerator save-current-keystroke)))
          (.add file-menu (menu-item "Save As..." #(save-as! frame tabs status)))
          (.add file-menu (menu-item "Save Audio..." #(save-audio! frame tabs status)))
          (.add file-menu
                (menu-item "Exit"
                           #(when (close-all-tabs! frame tabs status)
                              (close-live-process!)
                              (.dispose frame))))
          (.add preferences-menu
                (checkbox-menu-item "Remove Insert Comments"
                                    (:remove-insert-comments @state)
                                    #(do
                                       (swap! state assoc :remove-insert-comments %)
                                       (set-status! status
                                                    (if %
                                                      "insert comments: removed"
                                                      "insert comments: shown")))))
          (.add preferences-menu
                (checkbox-menu-item "Remove Playback Highlighting"
                                    (:remove-playback-highlighting @state)
                                    #(do
                                       (swap! state assoc :remove-playback-highlighting %)
                                       (when %
                                         (when-let [editor (active-editor tabs)]
                                           (clear-live-step-highlight! editor)))
                                       (set-status! status
                                                    (if %
                                                      "playback highlighting: removed"
                                                      "playback highlighting: shown")))))
          (.add preferences-menu
                (menu-item "Clear Null Parameters"
                           #(when-let [editor (active-editor tabs)]
                              (clear-null-parameters! editor status))))
          (.add audio-menu devices-menu)
          (.add audio-menu (menu-item "Refresh Devices" #(refresh-devices!)))
          (.add about-menu (menu-item "Language Reference" #(show-language-reference! frame)))
          (.add about-menu (menu-item "About MeScript" #(show-about! frame)))
          (.add menu-bar file-menu)
          (.add menu-bar preferences-menu)
          (.add menu-bar audio-menu)
          (.add menu-bar about-menu)
          (.setJMenuBar frame menu-bar)
          (refresh-devices!)))

      (let [editor (active-editor tabs)]
        (.putClientProperty (.getRootPane frame) "mescript.editor" editor)
        (.add (.getContentPane frame) tabs BorderLayout/CENTER)
        (.add (.getContentPane frame) tools BorderLayout/EAST)
        (.add (.getContentPane frame) status BorderLayout/SOUTH)))

    (.setDefaultCloseOperation frame JFrame/DO_NOTHING_ON_CLOSE)
    (.setName frame "mescript-workstation-frame")
    (.setName status "mescript-status")
    (.putClientProperty (.getRootPane frame) "mescript.editor-tabs" tabs)
    (.putClientProperty (.getRootPane frame) "mescript.status" status)
    (install-save-shortcut! frame tabs status)
    (.addWindowListener frame
                        (proxy [WindowAdapter] []
                          (windowClosing [_]
                            (when (close-all-tabs! frame tabs status)
                              (close-live-process!)
                              (.dispose frame)))))
    (.pack frame)
    (.setLocationRelativeTo frame nil)
    (.setVisible frame true)
    frame))

(defn -main [& args]
  (let [file (some-> (first args) File.)]
    (SwingUtilities/invokeLater #(build-ui file))))

(defn no-gui-mode? []
  (or (System/getenv "GLITCHLISP_NO_GUI")
      (System/getProperty "glitchlisp.noGui")
      (GraphicsEnvironment/isHeadless)))

(when-not (no-gui-mode?)
  (apply -main *command-line-args*))
