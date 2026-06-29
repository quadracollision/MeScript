(ns glitchlisp-swing
  (:require [clojure.edn :as edn]
            [clojure.java.io :as io]
            [clojure.set :as set]
            [clojure.string :as str])
  (:import
    [java.awt BorderLayout Color Component Container Cursor Dimension Font GraphicsEnvironment Window]
    [java.awt.event ActionListener ComponentAdapter FocusAdapter InputEvent KeyEvent MouseAdapter MouseEvent WindowAdapter]
    [java.io BufferedReader ByteArrayInputStream ByteArrayOutputStream File InputStreamReader OutputStreamWriter PushbackReader StringReader]
    [javax.sound.sampled AudioFileFormat$Type AudioFormat AudioInputStream AudioSystem Clip]
    [javax.swing.event CaretListener ChangeListener DocumentListener]
    [javax.swing.text DefaultHighlighter$DefaultHighlightPainter JTextComponent SimpleAttributeSet StyleConstants StyledDocument]
    [javax.swing AbstractAction BorderFactory Box BoxLayout ButtonGroup JButton JCheckBoxMenuItem JComboBox JFileChooser JComponent JFrame JLabel JMenu JMenuBar JMenuItem JOptionPane JPanel JPopupMenu JRadioButtonMenuItem JScrollPane JSplitPane JTabbedPane JTextField JTextPane KeyStroke ScrollPaneConstants SwingUtilities Timer UIManager]
    [javax.swing.border BevelBorder LineBorder]))

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
(def set-syntax-theme! glitchlisp.swing.editor/set-syntax-theme!)
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
(def track-param-items glitchlisp.swing.render/track-param-items)
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
(def send-compiled-live-queued-update! glitchlisp.swing.live/send-compiled-live-queued-update!)

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
(def live-auto-update-suppressed-key "glitchlisp.liveAutoUpdateSuppressed")

(defn valid-live-compiled-source [source]
  (let [preview (preview-source source)]
    (require-playback-form! preview)
    (compile-glitchlisp-source preview)))

(defn live-queue-update! [^JFrame frame ^JTextComponent editor ^JLabel status device source]
  (future
    (try
      (let [compiled (valid-live-compiled-source source)]
        (if (live-process-running?)
          (send-compiled-live-queued-update! editor status compiled "queued for next loop...")
          (live-update! frame editor status device)))
      (catch Exception ex
        (SwingUtilities/invokeLater
          #(do
             (report-source-error! editor status ex)
             (when-not (GraphicsEnvironment/isHeadless)
               (JOptionPane/showMessageDialog frame (clean-error-message ex) "Live update failed" JOptionPane/ERROR_MESSAGE))))))))

(defn next-live-auto-edit-token! []
  (:live-auto-edit-token
    (swap! state update :live-auto-edit-token
           #(inc (long (or % 0))))))

(defn cancel-live-auto-update! [^JTextComponent editor]
  (let [token (next-live-auto-edit-token!)]
    (.putClientProperty editor "glitchlisp.liveAutoUpdateToken" token)
    (when-let [^Timer timer (.getClientProperty editor live-auto-update-timer-key)]
      (.stop timer))
    token))

(defn live-auto-apply-source! [^JTextComponent editor ^JLabel status source token]
  (future
    (try
      (let [compiled (valid-live-compiled-source source)]
        (when (and (= token (:live-auto-edit-token @state))
                   (live-process-running?))
          (swap! state assoc :live-auto-last-error nil)
          (send-compiled-live-queued-update! editor status compiled "live edit queued for next loop")))
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
            (when-not (or (.getClientProperty editor syntax-refreshing-key)
                          (.getClientProperty editor live-auto-update-suppressed-key))
              (schedule-live-auto-update! editor status)))
          (removeUpdate [_ _]
            (when-not (or (.getClientProperty editor syntax-refreshing-key)
                          (.getClientProperty editor live-auto-update-suppressed-key))
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

(defn empty-untitled-editor? [^JTextComponent editor]
  (and editor
       (nil? (editor-file editor))
       (not (editor-dirty? editor))
       (str/blank? (.getText editor))))

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

(declare theme-option saved-theme-id)

(defn current-theme []
  (or (:theme-data @state)
      (theme-option (or (:theme @state) (saved-theme-id)))))

(defn refresh-tab-header-style! [^JTabbedPane tabs ^JTextComponent editor]
  (when-let [scroll (.getClientProperty editor "mescript.scroll")]
    (let [idx (.indexOfComponent tabs scroll)]
      (when (>= idx 0)
        (let [header (.getTabComponentAt tabs idx)
              theme (current-theme)
              selected? (= idx (.getSelectedIndex tabs))]
	          (when header
	            (.setOpaque ^JComponent header false)
	            (.setBorder ^JComponent header (BorderFactory/createEmptyBorder 2 5 2 4))
	            (.revalidate ^JComponent header)
	            (.repaint ^JComponent header))
	          (when-let [label (.getClientProperty editor "mescript.tab-label")]
	            (.setForeground ^JLabel label (if selected? (:text theme) (:muted theme))))
	          (when-let [close-label (.getClientProperty editor "mescript.tab-close-label")]
            (.setForeground ^JLabel close-label (if selected? (:text theme) (:muted theme)))))))))

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

(declare close-tab! close-main-window! dispose-child-windows!)
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
                            (.setForeground (:muted (current-theme)))
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
	          (.putClientProperty editor "mescript.tab-close-label" close-label)
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
	                                 (.setForeground close-label (:accent (current-theme))))
	                               (mouseExited [^MouseEvent _]
	                                 (refresh-tab-header-style! tabs editor))))
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

(defn save-editor-to-file! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status ^File file]
  (try
    (write-file! file (.getText editor))
    (set-editor-file! editor file)
    (set-editor-dirty! editor false)
    (refresh-tab-title! tabs editor)
    (set-status! status (str "saved " (.getPath file)))
    true
    (catch Exception ex
      (set-status! status (str "save failed: " (.getMessage ex)))
      (when frame
        (JOptionPane/showMessageDialog
          frame
          (str "Could not save " (.getPath file) "\n\n" (.getMessage ex))
          "Save Failed"
          JOptionPane/ERROR_MESSAGE))
      false)))

(defn save-editor! [^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status]
  (let [file (or (editor-file editor) (choose-file-for-editor frame "Save" editor))]
    (when file
      (save-editor-to-file! frame tabs editor status file))))

(declare retro-editor-font style-component!)

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
  (.setFont editor retro-editor-font)
  (when-let [theme (:theme-data @state)]
    (.setBackground editor (:field theme))
    (.setForeground editor (:text theme))
    (.setCaretColor editor (:accent theme)))
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
    (.setRowHeaderView scroll (line-number-gutter editor))
    (when-let [theme (:theme-data @state)]
      (style-component! scroll theme))
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

(defn close-tab!
  ([^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status]
   (close-tab! frame tabs editor status true))
  ([^JFrame frame ^JTabbedPane tabs ^JTextComponent editor ^JLabel status add-empty-tab?]
   (when (or (nil? frame)
             (prompt-save-tab! frame tabs editor status))
     (when-let [scroll (.getClientProperty editor "mescript.scroll")]
       (let [idx (.indexOfComponent tabs scroll)]
         (when (>= idx 0)
           (.removeTabAt tabs idx)
           (when (and add-empty-tab? (zero? (.getTabCount tabs)))
             (add-editor-tab! tabs (or status (JLabel.)) "" nil))
           (sync-active-file! tabs)
           (when status
             (set-status! status "closed tab"))
           true))))))

(defn close-current-tab! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when-let [editor (active-editor tabs)]
    (let [last-tab? (= 1 (.getTabCount tabs))]
      (when (close-tab! frame tabs editor status (not last-tab?))
        (when last-tab?
          (dispose-child-windows! frame)
          (close-live-process!)
          (.dispose frame))))))

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
      (save-editor-to-file! frame tabs editor status file))))

(defn bind-app-action! [^JComponent component ^KeyStroke keystroke action-key f]
  (.put (.getInputMap component JComponent/WHEN_IN_FOCUSED_WINDOW)
        keystroke
        action-key)
  (.put (.getActionMap component)
        action-key
        (proxy [AbstractAction] []
          (actionPerformed [_] (f)))))

(def save-current-keystroke
  (KeyStroke/getKeyStroke KeyEvent/VK_S InputEvent/CTRL_DOWN_MASK))

(def close-current-tab-keystroke
  (KeyStroke/getKeyStroke KeyEvent/VK_W InputEvent/CTRL_DOWN_MASK))

(defn install-app-shortcuts! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (bind-app-action! (.getRootPane frame)
                    save-current-keystroke
                    "mescript-save-current"
                    #(save-current! frame tabs status))
  (bind-app-action! (.getRootPane frame)
                    close-current-tab-keystroke
                    "mescript-close-current-tab"
                    #(close-current-tab! frame tabs status)))

(defn new-file! [^JTabbedPane tabs ^JLabel status]
  (add-editor-tab! tabs status "" nil)
  (set-status! status "new file"))

(defn replace-editor-with-file! [^JTabbedPane tabs ^JTextComponent editor ^File file source]
  (.putClientProperty editor "mescript.file" file)
  (.putClientProperty editor "mescript.tab-name" nil)
  (.setText editor source)
  (clear-editor-undo-history! editor)
  (set-editor-dirty! editor false)
  (refresh-syntax-colors! editor)
  (refresh-tab-title! tabs editor)
  (select-editor-tab! tabs editor)
  (sync-active-file! tabs)
  editor)

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
    (let [source (read-file file)]
      (if-let [editor (let [active (active-editor tabs)]
                        (when (empty-untitled-editor? active)
                          active))]
        (replace-editor-with-file! tabs editor file source)
        (add-editor-tab! tabs status source file))
      (set-status! status (str "opened " (.getPath file))))))

(defn save-audio-to-file! [^JFrame frame ^JTextComponent editor ^JLabel status ^File file]
  (render-audio! frame editor status (.getPath file) (.getText editor) false false))

(declare choose-wav-file)

(defn save-audio! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when-let [editor (active-editor tabs)]
    (when-let [file (choose-wav-file frame)]
      (save-audio-to-file! frame editor status file))))

(declare active-editor)

(def inspector-repl-frame-key "mescript.inspectorReplFrame")
(def inspector-repl-result-key "mescript.inspectorReplResult")
(def theme-selector-frame-key "mescript.themeSelectorFrame")

(def preferences-file-name "mescript-preferences.edn")
(def theme-preference-key "theme")
(def tools-width-preference-key "tools-width")
(def window-x-preference-key "window-x")
(def window-y-preference-key "window-y")
(def window-width-preference-key "window-width")
(def window-height-preference-key "window-height")
(def default-theme-id "retro-cherry")
(def default-tools-width 148)
(def default-window-width 741)
(def default-window-height 580)

(defn rgb [r g b]
  (Color. (int r) (int g) (int b)))

(defn bevel-border [theme]
  (BorderFactory/createBevelBorder
    BevelBorder/RAISED
    (:highlight theme)
    (:panel theme)
    (:shadow theme)
    (:background theme)))

(defn editor-border [theme]
  (LineBorder. (:shadow theme) 1))

(def theme-options
  [{:id "retro-green"
    :label "Green"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 27 31 31)
    :panel (rgb 47 54 54)
    :panel-alt (rgb 58 66 66)
    :field (rgb 12 17 14)
    :field-alt (rgb 31 38 34)
    :text (rgb 218 238 216)
    :muted (rgb 141 165 150)
    :accent (rgb 108 207 132)
    :highlight (rgb 120 142 135)
    :shadow (rgb 6 8 8)
    :comment (rgb 104 142 104)
    :string (rgb 126 210 147)
    :form (rgb 128 205 255)
    :keyword (rgb 255 202 96)
    :number (rgb 255 144 112)
    :note (rgb 210 166 255)}
   {:id "retro-amber"
    :label "Amber"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 30 27 22)
    :panel (rgb 57 49 38)
    :panel-alt (rgb 72 62 47)
    :field (rgb 18 13 8)
    :field-alt (rgb 40 32 22)
    :text (rgb 255 222 150)
    :muted (rgb 188 151 86)
    :accent (rgb 255 180 72)
    :highlight (rgb 143 117 73)
    :shadow (rgb 8 6 4)
    :comment (rgb 164 128 70)
    :string (rgb 240 196 116)
    :form (rgb 255 214 128)
    :keyword (rgb 255 156 72)
    :number (rgb 255 118 84)
    :note (rgb 224 172 255)}
   {:id "retro-phosphor"
    :label "Phosphor"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 5 18 10)
    :panel (rgb 12 38 22)
    :panel-alt (rgb 20 54 32)
    :field (rgb 0 10 4)
    :field-alt (rgb 8 30 16)
    :text (rgb 170 255 176)
    :muted (rgb 90 170 98)
    :accent (rgb 86 255 116)
    :highlight (rgb 74 132 82)
    :shadow (rgb 0 4 0)
    :comment (rgb 86 150 92)
    :string (rgb 130 245 150)
    :form (rgb 150 255 202)
    :keyword (rgb 210 255 132)
    :number (rgb 255 205 112)
    :note (rgb 185 190 255)}
   {:id "retro-midnight"
    :label "Midnight"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 12 14 24)
    :panel (rgb 27 31 48)
    :panel-alt (rgb 38 43 63)
    :field (rgb 5 7 14)
    :field-alt (rgb 20 24 38)
    :text (rgb 214 224 255)
    :muted (rgb 132 146 184)
    :accent (rgb 128 168 255)
    :highlight (rgb 78 92 132)
    :shadow (rgb 2 3 8)
    :comment (rgb 108 128 160)
    :string (rgb 126 220 190)
    :form (rgb 138 178 255)
    :keyword (rgb 255 198 110)
    :number (rgb 255 132 140)
    :note (rgb 210 154 255)}
   {:id "retro-cherry"
    :label "Cherry"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 28 12 18)
    :panel (rgb 55 24 34)
    :panel-alt (rgb 74 34 48)
    :field (rgb 14 5 9)
    :field-alt (rgb 42 18 28)
    :text (rgb 255 220 226)
    :muted (rgb 198 126 142)
    :accent (rgb 255 92 126)
    :highlight (rgb 142 70 86)
    :shadow (rgb 6 2 4)
    :comment (rgb 176 100 118)
    :string (rgb 255 168 146)
    :form (rgb 255 118 158)
    :keyword (rgb 255 210 118)
    :number (rgb 132 220 255)
    :note (rgb 218 162 255)}
   {:id "retro-blue"
    :label "Blue"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 10 22 32)
    :panel (rgb 24 46 64)
    :panel-alt (rgb 34 62 84)
    :field (rgb 2 12 20)
    :field-alt (rgb 18 36 52)
    :text (rgb 198 238 255)
    :muted (rgb 110 170 196)
    :accent (rgb 86 204 255)
    :highlight (rgb 72 116 138)
    :shadow (rgb 0 5 10)
   :comment (rgb 88 144 170)
   :string (rgb 122 242 226)
   :form (rgb 116 190 255)
   :keyword (rgb 255 214 108)
   :number (rgb 255 144 104)
   :note (rgb 194 160 255)}
   {:id "retro-oxide"
    :label "Oxide"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 18 20 20)
    :panel (rgb 38 42 42)
    :panel-alt (rgb 52 58 56)
    :field (rgb 8 10 10)
    :field-alt (rgb 26 30 29)
    :text (rgb 224 230 218)
    :muted (rgb 128 142 132)
    :accent (rgb 214 120 58)
    :highlight (rgb 94 104 96)
    :shadow (rgb 2 3 3)
    :comment (rgb 108 130 116)
    :string (rgb 130 214 154)
    :form (rgb 118 188 238)
    :keyword (rgb 238 174 82)
    :number (rgb 236 100 84)
    :note (rgb 194 144 242)}
   {:id "retro-crt"
    :label "CRT"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 7 18 20)
    :panel (rgb 13 42 44)
    :panel-alt (rgb 22 58 60)
    :field (rgb 0 8 9)
    :field-alt (rgb 8 29 31)
    :text (rgb 190 255 224)
    :muted (rgb 88 172 148)
    :accent (rgb 64 240 188)
    :highlight (rgb 50 118 112)
    :shadow (rgb 0 3 4)
    :comment (rgb 80 154 132)
    :string (rgb 114 255 168)
    :form (rgb 102 216 255)
    :keyword (rgb 254 230 112)
    :number (rgb 255 132 84)
    :note (rgb 190 154 255)}
   {:id "retro-plum"
    :label "Plum"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 22 17 28)
    :panel (rgb 42 32 54)
    :panel-alt (rgb 58 44 72)
    :field (rgb 10 6 16)
    :field-alt (rgb 31 22 42)
    :text (rgb 238 224 248)
    :muted (rgb 164 138 178)
    :accent (rgb 224 110 190)
    :highlight (rgb 102 78 118)
    :shadow (rgb 4 2 8)
    :comment (rgb 144 118 158)
    :string (rgb 134 224 182)
    :form (rgb 128 188 255)
    :keyword (rgb 255 204 104)
    :number (rgb 255 128 112)
    :note (rgb 224 152 255)}
   {:id "retro-steel"
    :label "Steel"
    :class "com.formdev.flatlaf.FlatDarkLaf"
    :background (rgb 20 24 28)
    :panel (rgb 40 48 54)
    :panel-alt (rgb 54 64 72)
    :field (rgb 9 12 16)
    :field-alt (rgb 28 34 40)
    :text (rgb 226 234 238)
    :muted (rgb 136 154 164)
    :accent (rgb 116 178 204)
    :highlight (rgb 92 108 118)
    :shadow (rgb 3 5 7)
    :comment (rgb 112 136 146)
    :string (rgb 126 218 170)
    :form (rgb 130 188 255)
    :keyword (rgb 246 198 92)
    :number (rgb 238 116 96)
    :note (rgb 200 154 246)}
   {:id "retro-bone"
    :label "Bone"
    :class "com.formdev.flatlaf.FlatLightLaf"
    :background (rgb 190 190 178)
    :panel (rgb 172 172 160)
    :panel-alt (rgb 222 222 210)
    :field (rgb 235 235 224)
    :field-alt (rgb 198 198 186)
    :text (rgb 10 12 12)
    :muted (rgb 64 66 62)
    :accent (rgb 94 82 66)
    :highlight (rgb 252 252 238)
    :shadow (rgb 56 58 54)
    :comment (rgb 70 88 68)
    :string (rgb 0 86 66)
    :form (rgb 66 58 126)
    :keyword (rgb 0 72 108)
    :number (rgb 116 54 0)
    :note (rgb 110 40 98)}
   {:id "retro-paper"
    :label "Paper"
    :class "com.formdev.flatlaf.FlatLightLaf"
    :background (rgb 215 213 200)
    :panel (rgb 196 194 180)
    :panel-alt (rgb 232 230 216)
    :field (rgb 250 248 232)
    :field-alt (rgb 204 202 188)
    :text (rgb 14 17 18)
    :muted (rgb 80 78 70)
    :accent (rgb 126 55 18)
    :highlight (rgb 255 254 238)
    :shadow (rgb 70 68 58)
    :comment (rgb 76 100 66)
    :string (rgb 0 96 72)
    :form (rgb 82 48 138)
    :keyword (rgb 0 76 116)
    :number (rgb 132 56 0)
    :note (rgb 126 34 104)}
   {:id "retro-light"
    :label "Light"
    :class "com.formdev.flatlaf.FlatLightLaf"
    :background (rgb 214 216 210)
    :panel (rgb 194 197 190)
    :panel-alt (rgb 238 240 234)
    :field (rgb 252 253 246)
    :field-alt (rgb 218 221 214)
    :text (rgb 8 12 14)
    :muted (rgb 72 76 76)
    :accent (rgb 0 84 112)
    :highlight (rgb 255 255 250)
    :shadow (rgb 62 66 66)
    :comment (rgb 70 96 70)
    :string (rgb 0 92 68)
    :form (rgb 74 54 142)
    :keyword (rgb 0 76 124)
    :number (rgb 126 54 0)
    :note (rgb 120 42 118)}])

(defn preferences-file []
  (child-file (app-dir) preferences-file-name))

(defn read-app-preferences []
  (let [file (preferences-file)]
    (if (.exists file)
      (try
        (let [value (edn/read-string (slurp file))]
          (if (map? value) value {}))
        (catch Exception _ {}))
      {})))

(defn write-app-preferences! [prefs]
  (try
    (spit (preferences-file) (pr-str prefs))
    true
    (catch Exception _ false)))

(defn theme-option [theme-id]
  (or (some #(when (= (:id %) theme-id) %) theme-options)
      (first theme-options)))

(defn saved-theme-id []
  (or (get (read-app-preferences) (keyword theme-preference-key))
      default-theme-id))

(defn save-theme-id! [theme-id]
  (write-app-preferences! (assoc (read-app-preferences)
                                 (keyword theme-preference-key)
                                 theme-id)))

(defn saved-tools-width []
  (let [value (get (read-app-preferences) (keyword tools-width-preference-key))]
    (if (number? value)
      (max 96 (min 420 (int value)))
      default-tools-width)))

(defn save-tools-width! [width]
  (when (number? width)
    (write-app-preferences! (assoc (read-app-preferences)
                                   (keyword tools-width-preference-key)
                                   (max 96 (min 420 (int width)))))))

(defn set-tools-panel-width! [^JSplitPane split width]
  (when (and split (number? width))
    (let [split-width (.getWidth split)
          target (max 96 (min 420 (int width)))
          divider (.getDividerSize split)]
      (when (pos? split-width)
        (.setDividerLocation split (max 0 (- split-width target divider)))))))

(defn saved-window-bounds []
  (let [prefs (read-app-preferences)
        x (get prefs (keyword window-x-preference-key))
        y (get prefs (keyword window-y-preference-key))
        width (get prefs (keyword window-width-preference-key))
        height (get prefs (keyword window-height-preference-key))]
    (when (and (number? x)
               (number? y)
               (number? width)
               (number? height)
               (>= width 520)
               (>= height 360))
      {:x (int x)
       :y (int y)
       :width (int width)
       :height (int height)})))

(defn save-window-bounds! [^JFrame frame]
  (when (and frame (.isDisplayable frame))
    (let [prefs (read-app-preferences)]
      (write-app-preferences!
        (assoc prefs
               (keyword window-x-preference-key) (.getX frame)
               (keyword window-y-preference-key) (.getY frame)
               (keyword window-width-preference-key) (.getWidth frame)
               (keyword window-height-preference-key) (.getHeight frame))))))

(defn clear-layout-preferences! []
  (write-app-preferences!
    (apply dissoc
           (read-app-preferences)
           (map keyword [tools-width-preference-key
                         window-x-preference-key
                         window-y-preference-key
                         window-width-preference-key
                         window-height-preference-key]))))

(def retro-ui-font (Font. Font/MONOSPACED Font/BOLD 12))
(def retro-editor-font (Font. Font/MONOSPACED Font/BOLD 14))

(defn put-ui-color! [key value]
  (UIManager/put key value))

(defn put-ui-int! [key value]
  (UIManager/put key (Integer/valueOf (int value))))

(defn apply-ui-manager-theme! [theme]
  (put-ui-color! "Panel.background" (:background theme))
  (put-ui-color! "MenuBar.background" (:background theme))
  (put-ui-color! "MenuBar.foreground" (:text theme))
  (put-ui-color! "Menu.background" (:background theme))
  (put-ui-color! "Menu.foreground" (:text theme))
  (put-ui-color! "Menu.selectionBackground" (:panel-alt theme))
  (put-ui-color! "Menu.selectionForeground" (:text theme))
  (put-ui-color! "MenuItem.background" (:background theme))
  (put-ui-color! "MenuItem.foreground" (:text theme))
  (put-ui-color! "MenuItem.selectionBackground" (:panel-alt theme))
  (put-ui-color! "MenuItem.selectionForeground" (:text theme))
  (put-ui-color! "PopupMenu.background" (:background theme))
  (put-ui-color! "PopupMenu.foreground" (:text theme))
  (put-ui-color! "RadioButtonMenuItem.background" (:background theme))
  (put-ui-color! "RadioButtonMenuItem.foreground" (:text theme))
  (put-ui-color! "RadioButtonMenuItem.selectionBackground" (:panel-alt theme))
  (put-ui-color! "RadioButtonMenuItem.selectionForeground" (:text theme))
  (put-ui-color! "CheckBoxMenuItem.background" (:background theme))
  (put-ui-color! "CheckBoxMenuItem.foreground" (:text theme))
  (put-ui-color! "CheckBoxMenuItem.selectionBackground" (:panel-alt theme))
  (put-ui-color! "CheckBoxMenuItem.selectionForeground" (:text theme))
  (put-ui-color! "Button.background" (:panel-alt theme))
  (put-ui-color! "Button.foreground" (:text theme))
  (put-ui-color! "Button.borderColor" (:shadow theme))
  (put-ui-color! "Button.focusedBorderColor" (:highlight theme))
  (put-ui-color! "Button.hoverBorderColor" (:highlight theme))
  (put-ui-color! "Button.focusedBackground" (:panel-alt theme))
  (put-ui-color! "Button.default.background" (:panel-alt theme))
  (put-ui-color! "Button.default.foreground" (:text theme))
  (put-ui-color! "Button.default.borderColor" (:shadow theme))
  (put-ui-color! "Button.default.focusedBorderColor" (:highlight theme))
  (put-ui-color! "Button.default.hoverBorderColor" (:highlight theme))
  (put-ui-color! "ComboBox.background" (:field-alt theme))
  (put-ui-color! "ComboBox.foreground" (:text theme))
  (put-ui-color! "Label.foreground" (:text theme))
  (put-ui-color! "OptionPane.background" (:background theme))
  (put-ui-color! "OptionPane.foreground" (:text theme))
  (put-ui-color! "OptionPane.messageForeground" (:text theme))
  (put-ui-color! "OptionPane.buttonAreaBackground" (:background theme))
  (put-ui-color! "OptionPane.messageAreaBackground" (:background theme))
  (put-ui-color! "OptionPane.borderColor" (:shadow theme))
  (put-ui-color! "TabbedPane.background" (:background theme))
  (put-ui-color! "TabbedPane.foreground" (:text theme))
  (put-ui-color! "TabbedPane.selectedBackground" (:panel theme))
  (put-ui-color! "TabbedPane.selectedForeground" (:text theme))
  (put-ui-color! "TabbedPane.inactiveForeground" (:muted theme))
  (put-ui-color! "TabbedPane.disabledForeground" (:muted theme))
  (put-ui-color! "TabbedPane.tabAreaBackground" (:background theme))
  (put-ui-color! "TabbedPane.hoverColor" (:panel-alt theme))
  (put-ui-color! "TextComponent.background" (:field theme))
  (put-ui-color! "TextComponent.foreground" (:text theme))
  (put-ui-color! "TextField.background" (:field theme))
  (put-ui-color! "TextField.foreground" (:text theme))
  (put-ui-color! "TextPane.background" (:field theme))
  (put-ui-color! "TextPane.foreground" (:text theme))
  (put-ui-color! "ScrollPane.background" (:background theme))
  (put-ui-color! "Viewport.background" (:field theme))
  (put-ui-color! "Component.focusColor" (:accent theme))
  (put-ui-color! "Component.focusedBorderColor" (:shadow theme))
  (put-ui-color! "Component.borderColor" (:shadow theme))
  (put-ui-color! "Component.custom.borderColor" (:shadow theme))
  (put-ui-color! "TextComponent.focusedBorderColor" (:shadow theme))
  (put-ui-color! "TextComponent.borderColor" (:shadow theme))
  (put-ui-color! "ScrollPane.borderColor" (:shadow theme))
  (put-ui-color! "TabbedPane.focusColor" (:accent theme))
  (put-ui-color! "TabbedPane.underlineColor" (:accent theme))
  (doseq [key ["Button.arc" "Component.arc" "CheckBox.arc" "ComboBox.arc" "TextComponent.arc" "TabbedPane.tabArc"]]
    (put-ui-int! key 0))
  (doseq [key ["defaultFont" "Button.font" "ComboBox.font" "Label.font" "Menu.font" "MenuItem.font" "TabbedPane.font"]]
    (UIManager/put key retro-ui-font)))

(defn style-button! [^JButton button theme]
  (.setFont button retro-ui-font)
  (.setBackground button (:panel-alt theme))
  (.setForeground button (:text theme))
  (.setFocusPainted button false)
  (.setBorderPainted button true)
  (.setContentAreaFilled button true)
  (.setOpaque button true)
  (.setBorder button (bevel-border theme)))

(declare style-component!)

(defn style-menu-item! [^JMenuItem item theme]
  (.setFont item retro-ui-font)
  (.setBackground item (:background theme))
  (.setForeground item (:text theme))
  (.setOpaque item true)
  (.setBorder item (BorderFactory/createEmptyBorder 3 8 3 8)))

(defn style-popup-menu! [^JPopupMenu popup theme]
  (.setBackground popup (:background theme))
  (.setForeground popup (:text theme))
  (.setBorder popup (LineBorder. (:shadow theme) 1))
  (doseq [child (.getComponents popup)]
    (style-component! child theme)))

(defn style-menu! [^JMenu menu theme]
  (style-menu-item! menu theme)
  (style-popup-menu! (.getPopupMenu menu) theme))

(defn style-component! [^Component component theme]
  (cond
    (instance? JButton component)
    (style-button! component theme)

    (instance? JMenu component)
    (style-menu! component theme)

    (instance? JMenuItem component)
    (style-menu-item! component theme)

    (instance? JPopupMenu component)
    (style-popup-menu! component theme)

    (instance? JMenuBar component)
    (doto ^JMenuBar component
      (.setBackground (:background theme))
      (.setForeground (:text theme))
      (.setBorder (LineBorder. (:shadow theme) 1)))

    (instance? JComboBox component)
    (doto ^JComboBox component
      (.setFont retro-ui-font)
      (.setBackground (:field-alt theme))
      (.setForeground (:text theme)))

    (instance? JLabel component)
    (doto ^JLabel component
      (.setFont retro-ui-font)
      (.setForeground (:text theme)))

    (instance? JTextPane component)
    (let [name (.getName component)
          editor? (= name "mescript-editor")
          gutter? (= name "mescript-line-numbers")]
      (doto ^JTextPane component
        (.setFont (if editor? retro-editor-font retro-ui-font))
        (.setBackground (if gutter? (:field-alt theme) (:field theme)))
        (.setForeground (if gutter? (:muted theme) (:text theme)))
        (.setCaretColor (:accent theme))
        (.setSelectedTextColor (:background theme))
        (.setSelectionColor (:accent theme))
        (.setBorder (BorderFactory/createEmptyBorder 0 6 0 6)))
      (when editor?
        (refresh-syntax-colors! component)))

    (instance? JTextField component)
    (doto ^JTextField component
      (.setFont retro-ui-font)
      (.setBackground (:field theme))
      (.setForeground (:text theme))
      (.setCaretColor (:accent theme))
      (.setBorder (bevel-border theme)))

    (instance? JTabbedPane component)
    (doto ^JTabbedPane component
      (.setFont retro-ui-font)
      (.setBackground (:background theme))
      (.setForeground (:text theme)))

    (instance? JSplitPane component)
    (doto ^JSplitPane component
      (.setBackground (:background theme))
      (.setForeground (:accent theme)))

    (instance? JComponent component)
    (doto ^JComponent component
      (.setBackground (:background theme))
      (.setForeground (:text theme))))
  (when (instance? JScrollPane component)
    (let [scroll ^JScrollPane component]
      (.setBackground scroll (:background theme))
      (.setBorder scroll (editor-border theme))
      (.setBackground (.getViewport scroll) (:field theme))
      (when-let [row-header (.getRowHeader scroll)]
        (.setBackground row-header (:field-alt theme)))))
  (when (instance? Container component)
    (doseq [child (.getComponents ^Container component)]
      (style-component! child theme))))

(defn apply-component-theme! [theme]
  (set-syntax-theme! theme)
  (doseq [^Window window (Window/getWindows)]
    (style-component! window theme)))

(defn refresh-window-themes! []
  (doseq [^Window window (Window/getWindows)]
    (SwingUtilities/updateComponentTreeUI window)
    (when-let [theme (:theme-data @state)]
      (style-component! window theme))
    (.revalidate window)
    (.repaint window)))

(defn apply-theme-id! [theme-id]
  (let [{:keys [id class]} (theme-option theme-id)]
    (try
      (Class/forName class)
      (UIManager/setLookAndFeel class)
      (apply-ui-manager-theme! (theme-option id))
      (save-theme-id! id)
      (swap! state assoc :theme id :theme-data (theme-option id))
      (apply-component-theme! (theme-option id))
      (refresh-window-themes!)
      {:ok true :theme id}
      (catch Exception ex
        {:ok false :theme id :message (.getMessage ex)}))))

(defn apply-saved-theme! []
  (apply-theme-id! (saved-theme-id)))

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

(defn inspector-tokenize-command [command]
  (->> (str/split (str/trim command) #"\s+")
       (remove str/blank?)
       vec))

(defn inspector-param-key [token]
  (let [token (str token)]
    (if (str/starts-with? token ":")
      token
      (str ":" token))))

(defn inspector-def-range-map [source]
  (reduce
    (fn [defs [start end]]
      (let [text (subs source start (inc end))]
        (try
          (let [form (first (read-source-forms text))
                def-symbol (second form)]
            (if (symbol? def-symbol)
              (assoc defs (str def-symbol)
                     {:kind :def
                      :symbol def-symbol
                      :source-start start
                      :source-end end
                      :text text
                      :form form})
              defs))
          (catch Exception _
            defs))))
    {}
    (form-ranges source "def")))

(defn inspector-top-level-form-ranges [source form-name]
  (let [scene-ranges (concat (form-ranges source "scene")
                             (form-ranges source "block")
                             (form-ranges source "def"))]
    (->> (form-ranges source form-name)
         (remove (fn [[start _]] (inside-any-range? start scene-ranges)))
         vec)))

(defn inspector-track-form? [form]
  (and (seq? form)
       (#{'d 'sample} (first form))
       (keyword? (second form))))

(defn inspector-track-key [track-id]
  (if (str/starts-with? track-id ":")
    (subs track-id 1)
    track-id))

(defn inspector-track-entry [source start end form extra]
  (let [track-id (name (second form))
        text (subs source start (inc end))]
    (merge {:kind :track
            :symbol nil
            :track-id track-id
            :source-start start
            :source-end end
            :track-source-start start
            :track-source-end end
            :text text
            :form form
            :track-form form}
           extra)))

(defn inspector-nested-track-ranges [source start end]
  (let [visible (code-visible-text source)]
    (loop [idx start
           ranges []]
      (if (>= idx end)
        ranges
        (let [d-found (.indexOf visible "(d" idx)
              sample-found (.indexOf visible "(sample" idx)
              candidates (->> [d-found sample-found]
                              (filter #(and (>= % 0) (<= % end)))
                              sort)
              open (first candidates)]
          (if-not open
            ranges
            (if (or (form-symbol-at? visible open "d")
                    (form-symbol-at? visible open "sample"))
              (if-let [close (matching-close source open \( \))]
                (recur (inc close) (if (<= close end)
                                     (conj ranges [open close])
                                     ranges))
                (recur (inc open) ranges))
              (recur (inc open) ranges))))))))

(defn inspector-assoc-track [tracks key entry]
  (if (contains? tracks key)
    tracks
    (assoc tracks key entry)))

(defn inspector-assoc-track-aliases [tracks track-id entry]
  (let [bare (name track-id)
        keyed (str ":" bare)]
    (-> tracks
        (inspector-assoc-track bare entry)
        (inspector-assoc-track keyed entry))))

(defn inspector-track-range-map [source]
  (let [defs (inspector-def-range-map source)
        def-tracks (reduce-kv
                     (fn [tracks def-name info]
                       (let [value (nth (:form info) 2 nil)]
                         (if (inspector-track-form? value)
                           (assoc tracks def-name
                                  (assoc info
                                         :track-id (name (second value))
                                         :track-form value
                                         :track-source-start (:source-start info)
                                         :track-source-end (:source-end info)))
                           tracks)))
                     {}
                     defs)
        top-level-tracks (reduce
                           (fn [tracks [start end]]
                             (let [text (subs source start (inc end))]
                               (try
	                                 (let [form (first (read-source-forms text))]
	                                   (if (inspector-track-form? form)
	                                     (inspector-assoc-track-aliases
	                                       tracks
	                                       (second form)
	                                       (inspector-track-entry source start end form {}))
	                                     tracks))
                                 (catch Exception _
                                   tracks))))
                           {}
                           (inspector-top-level-form-ranges source "d"))
        top-level-samples (reduce
                            (fn [tracks [start end]]
                              (let [text (subs source start (inc end))]
                                (try
                                  (let [form (first (read-source-forms text))]
                                    (if (inspector-track-form? form)
                                      (inspector-assoc-track-aliases
                                        tracks
                                        (second form)
                                        (inspector-track-entry source start end form {}))
                                      tracks))
                                  (catch Exception _
                                    tracks))))
                            {}
                            (inspector-top-level-form-ranges source "sample"))
        nested-tracks (reduce
                        (fn [tracks [_ info]]
                          (reduce
                            (fn [tracks [start end]]
                              (let [text (subs source start (inc end))]
                                (try
                                  (let [form (first (read-source-forms text))]
                                    (if (inspector-track-form? form)
                                      (inspector-assoc-track-aliases
                                        tracks
                                        (second form)
                                        (inspector-track-entry
                                          source start end form
                                          {:container-symbol (:symbol info)
                                           :container-source-start (:source-start info)
                                           :container-source-end (:source-end info)}))
                                      tracks))
                                  (catch Exception _
                                    tracks))))
                            tracks
                            (inspector-nested-track-ranges source
                                                           (:source-start info)
                                                           (:source-end info))))
                        {}
                        defs)]
    (merge top-level-tracks top-level-samples def-tracks nested-tracks)))

(declare inspector-editable-result)

(defn inspector-scene-range-map [source]
  (reduce
    (fn [scenes [start end]]
      (let [text (subs source start (inc end))]
        (try
          (let [form (first (read-source-forms text))
                scene-id (second form)]
            (if (keyword? scene-id)
              (assoc scenes (name scene-id)
                     {:kind :scene
                      :scene-id scene-id
                      :source-start start
                      :source-end end
                      :text text
                      :form form})
              scenes))
          (catch Exception _
            scenes))))
    {}
    (form-ranges source "scene")))

(defn inspector-scene-source-result [source target]
  (let [key (if (str/starts-with? target ":") (subs target 1) target)
        scenes (inspector-scene-range-map source)]
    (if-let [{:keys [source-start source-end text scene-id]} (get scenes key)]
      (inspector-editable-result :scene source source-start source-end text {:scene-id scene-id})
      {:text (str "unknown scene " target)
       :editable false})))

(defn inspector-bpm-result [source]
  (let [ranges (inspector-top-level-form-ranges source "bpm")]
    (if-let [[start end] (last ranges)]
      (let [text (subs source start (inc end))
            form (first (read-source-forms text))
            value (second form)
            value-text (emit-form value)
            value-start (.indexOf source value-text start)]
        (if (and (>= value-start 0) (<= value-start end))
          (inspector-editable-result :bpm source value-start (dec (+ value-start (count value-text))) value-text {})
          {:text (str "bpm " (bpm-from-source source))
           :editable false}))
      {:text (str "bpm " (bpm-from-source source) "\nno top-level (bpm N) form found")
       :editable false})))

(defn inspector-token-range [text offset limit]
  (let [visible (code-visible-text text)
        offset (loop [idx offset]
                 (if (and (< idx limit)
                          (Character/isWhitespace (.charAt visible idx)))
                   (recur (inc idx))
                   idx))]
    (when (< offset limit)
      (let [ch (.charAt visible offset)]
        (cond
          (= ch \()
          (when-let [close (matching-close text offset \( \))]
            [offset close])

          (= ch \[)
          (when-let [close (matching-close text offset \[ \])]
            [offset close])

          :else
          [offset
           (dec (loop [idx offset]
                  (if (and (< idx limit)
                           (let [ch (.charAt visible idx)]
                             (not (or (Character/isWhitespace ch)
                                      (contains? #{\( \) \[ \] \;} ch)))))
                    (recur (inc idx))
                    idx)))])))))

(defn inspector-find-track-form-range [source start end]
  (let [visible (code-visible-text source)]
    (loop [idx start]
      (when-let [open (let [found (.indexOf visible "(d" idx)]
                        (when (and (>= found 0) (<= found end)) found))]
        (if (form-symbol-at? visible open "d")
          (when-let [close (matching-close source open \( \))]
            (when (<= close end)
              [open close]))
          (recur (inc open)))))))

(defn inspector-param-value-range [source track-start track-end param-key]
  (let [visible (code-visible-text source)
        needle (str param-key)
        limit (inc track-end)]
    (loop [idx (inc track-start)
           depth 0
           in-string? false
           escape? false
           in-comment? false]
      (when (< idx limit)
        (let [ch (.charAt visible idx)]
          (cond
            in-comment?
            (recur (inc idx) depth false false (not= ch \newline))

            escape?
            (recur (inc idx) depth in-string? false false)

            (and in-string? (= ch \\))
            (recur (inc idx) depth true true false)

            (= ch \")
            (recur (inc idx) depth (not in-string?) false false)

            in-string?
            (recur (inc idx) depth true false false)

            (= ch \;)
            (recur (inc idx) depth false false true)

            (= ch \()
            (recur (inc idx) (inc depth) false false false)

            (= ch \))
            (recur (inc idx) (dec depth) false false false)

            (= ch \[)
            (recur (inc idx) (inc depth) false false false)

            (= ch \])
            (recur (inc idx) (dec depth) false false false)

	            (and (= depth 0)
	                 (= needle (subs visible idx (min limit (+ idx (count needle)))))
                 (or (= (+ idx (count needle)) limit)
                     (Character/isWhitespace (.charAt visible (+ idx (count needle))))))
            (inspector-token-range source (+ idx (count needle)) limit)

            :else
            (recur (inc idx) depth false false false)))))))

(defn inspector-track-param-value [track-form param-key]
  (let [target (keyword (subs param-key 1))]
    (loop [items (nnext track-form)]
      (when (seq items)
        (if (= target (first items))
          (second items)
          (recur (next items)))))))

(defn inspector-editable-result [kind source start end text extra]
  (merge {:kind kind
          :source-start start
          :source-end end
          :text text
          :editable true}
         extra))

(defn inspector-symbol-result [source sym]
  (let [defs (inspector-def-range-map source)
        tracks (inspector-track-range-map source)]
    (cond
      (contains? defs sym)
      (let [{:keys [source-start source-end text symbol]} (get defs sym)]
        (inspector-editable-result :def source source-start source-end text {:symbol symbol}))

      (contains? tracks sym)
      (let [{:keys [source-start source-end text symbol track-id]} (get tracks sym)]
        (inspector-editable-result (if symbol :def :track)
                                   source source-start source-end text
                                   {:symbol symbol :track-id track-id}))

      :else
      {:text (str "unknown symbol '" sym "'")
       :editable false})))

(defn inspector-symbol-param-result [source sym param-key]
  (let [defs (inspector-def-range-map source)
        tracks (inspector-track-range-map source)
        track (get tracks sym)]
    (if-not track
      {:text (str "unknown track or def '" sym "'")
       :editable false}
      (let [value (inspector-track-param-value (:track-form track) param-key)]
        (cond
          (nil? value)
          {:text (str sym " has no " param-key)
           :editable false}

          (and (symbol? value)
               (contains? defs (name value)))
          (let [{:keys [source-start source-end text symbol]} (get defs (name value))]
            (inspector-editable-result :def source source-start source-end text {:symbol symbol}))

          :else
          (let [[track-start track-end] (or (and (:symbol track)
                                                 (inspector-find-track-form-range source
                                                                                  (:source-start track)
                                                                                  (:source-end track)))
                                            [(:track-source-start track)
                                             (:track-source-end track)])
                [value-start value-end] (inspector-param-value-range source
                                                                     track-start
                                                                     track-end
                                                                     param-key)
                text (if (and value-start value-end)
                       (subs source value-start (inc value-end))
                       (emit-form value))]
            (if (and value-start value-end)
              (inspector-editable-result :value source value-start value-end text
                                         {:symbol (:symbol track)
                                          :track-id (:track-id track)
                                          :param param-key})
              {:text text
               :editable false})))))))

(defn inspector-compile-expanded-forms [source tail-form]
  (->> (str source "\n" tail-form "\n")
       compile-glitchlisp-source
       read-source-forms))

(defn inspector-expand-symbol-result [source sym]
  (try
    (let [forms (inspector-compile-expanded-forms source sym)]
      {:text (if-let [form (last forms)]
               (emit-form form)
               (str "no expanded form for " sym))
       :editable false})
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-expand-param-result [source sym param-key]
  (try
    (let [forms (inspector-compile-expanded-forms source sym)
          form (last forms)
          value (when (and (seq? form) (= 'd (first form)))
                  (inspector-track-param-value form param-key))]
      {:text (if value
               (emit-form value)
               (str sym " has no " param-key " after expansion"))
       :editable false})
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-compiled-forms [source]
  (-> source compile-glitchlisp-source read-source-forms))

(defn inspector-compiled-scenes [source]
  (->> (inspector-compiled-forms source)
       (filter scene-form?)
       (map (fn [form] [(scene-name form) form]))
       (into {})))

(defn inspector-compiled-tracks [source]
  (->> (inspector-compiled-forms source)
       (mapcat (fn [form]
                 (cond
                   (top-level-track? form) [form]
                   (scene-form? form) (filter top-level-track? (scene-body-forms form))
                   :else [])))
       (map (fn [form] [(track-id form) form]))
       (into {})))

(defn inspector-list-result [source kind]
  (try
    {:text (case kind
             "defs" (str/join "\n" (sort (keys (inspector-def-range-map source))))
             "tracks" (str/join "\n" (sort (map #(str %) (keys (inspector-compiled-tracks source)))))
             "scenes" (str/join "\n" (sort (map #(str %) (keys (inspector-compiled-scenes source))))))
     :editable false}
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-track-info-text [track-form]
  (let [items (track-param-items track-form)
        gate (or (pair-value items :gate) 1)
        note (or (pair-value items :note) 'c3)
        gate-summary (gate-pattern-summary gate)
        note-summary (note-pattern-summary note)
        loop-steps (track-loop-steps track-form)]
    (str "track " (track-id track-form) "\n"
         "loop steps: " loop-steps "\n"
         "gate length: " (:length gate-summary) "\n"
         "gate hits: " (:hits gate-summary) "\n"
         "gate slots: " (:slots gate-summary) "\n"
         "note length: " (:length note-summary) "\n"
         "note mode: " (name (:mode note-summary)))))

(defn inspector-scene-track-lines [scene-form]
  (->> (scene-body-forms scene-form)
       (filter top-level-track?)
       (map (fn [track-form]
              (let [items (track-param-items track-form)
                    gate-summary (gate-pattern-summary (or (pair-value items :gate) 1))
                    note-summary (note-pattern-summary (or (pair-value items :note) 'c3))]
                (str "  " (track-id track-form)
                     " gate=" (:length gate-summary)
                     " hits=" (:hits gate-summary)
                     " notes=" (:length note-summary)
                     " note-mode=" (name (:mode note-summary))))))))

(defn inspector-scene-info-text [scene-form]
  (let [declared (or (scene-option-value scene-form :steps)
                     (scene-option-value scene-form :length))
        effective (scene-steps-from-form scene-form)
        inferred (try
                   (scene-inferred-steps scene-form)
                   (catch Exception _ nil))
        repeat-count (scene-repeat-from-form scene-form)
        lines (inspector-scene-track-lines scene-form)]
    (str "scene " (scene-name scene-form) "\n"
         "declared steps: " (or declared "-") "\n"
         "effective steps: " effective "\n"
         "inferred full loop: " (or inferred "-") "\n"
         "repeat: " (if (zero? repeat-count) "loop" repeat-count) "\n"
         "\ntracks:\n"
         (if (seq lines) (str/join "\n" lines) "  -"))))

(defn inspector-info-result [source target]
  (try
    (cond
      (str/starts-with? target ":")
      (let [scene-key (keyword (subs target 1))
            scenes (inspector-compiled-scenes source)]
        (if-let [scene-form (get scenes scene-key)]
          {:text (inspector-scene-info-text scene-form) :editable false}
          {:text (str "unknown scene " target) :editable false}))

      :else
      (let [tracks (inspector-compiled-tracks (str source "\n" target "\n"))
            form (last (inspector-compile-expanded-forms source target))]
        (cond
          (top-level-track? form)
          {:text (inspector-track-info-text form) :editable false}

          :else
          (let [summary (gate-pattern-summary form)]
            {:text (str target "\n"
                        "steps: " (:length summary) "\n"
                        "hits: " (:hits summary) "\n"
                        "slots: " (:slots summary))
             :editable false}))))
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-length-result [source target]
  (try
    (if (str/starts-with? target ":")
      (let [scene-key (keyword (subs target 1))
            scene-form (get (inspector-compiled-scenes source) scene-key)]
        {:text (if scene-form
                 (str (scene-name scene-form) "\n"
                      "effective steps: " (scene-steps-from-form scene-form) "\n"
                      "total repeated steps: " (scene-total-steps-from-form scene-form))
                 (str "unknown scene " target))
         :editable false})
      (let [form (last (inspector-compile-expanded-forms source target))]
        {:text (if (top-level-track? form)
                 (str target "\nloop steps: " (track-loop-steps form))
                 (let [summary (gate-pattern-summary form)]
                   (str target "\nsteps: " (:length summary))))
         :editable false}))
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-hits-result [source target]
  (try
    (let [form (last (inspector-compile-expanded-forms source target))
          gate (if (top-level-track? form)
                 (or (pair-value (track-param-items form) :gate) 1)
                 form)
          summary (gate-pattern-summary gate)]
      {:text (str target "\n"
                  "steps: " (:length summary) "\n"
                  "hits: " (:hits summary) "\n"
                  "slots: " (:slots summary))
       :editable false})
    (catch Exception ex
      {:text (clean-error-message ex)
       :editable false})))

(defn inspector-command-result [source command]
  (let [tokens (inspector-tokenize-command command)]
    (case (count tokens)
      0 {:text "" :editable false}
      1 (if (#{"defs" "tracks" "scenes"} (tokens 0))
          (inspector-list-result source (tokens 0))
          (if (= "bpm" (tokens 0))
            (inspector-bpm-result source)
            (inspector-symbol-result source (tokens 0))))
	      2 (case (tokens 0)
	          "expand" (inspector-expand-symbol-result source (tokens 1))
	          "info" (inspector-info-result source (tokens 1))
	          "scene" (inspector-scene-source-result source (tokens 1))
	          "length" (inspector-length-result source (tokens 1))
          "hits" (inspector-hits-result source (tokens 1))
          "replace" (inspector-symbol-result source (tokens 1))
          (inspector-symbol-param-result source (tokens 0) (inspector-param-key (tokens 1))))
      3 (case (tokens 0)
          "expand" (inspector-expand-param-result source (tokens 1) (inspector-param-key (tokens 2)))
          "replace" (inspector-symbol-param-result source (tokens 1) (inspector-param-key (tokens 2)))
          {:text "unsupported command" :editable false})
      {:text "unsupported command" :editable false})))

(defn inspector-validate-replacement! [source start end replacement]
  (let [candidate (str (subs source 0 start)
                       replacement
                       (subs source (inc end)))]
    (valid-live-compiled-source candidate)
    candidate))

(defn inspector-apply-result! [^JFrame frame ^JTabbedPane tabs ^JTextComponent result-editor ^JLabel inspector-status]
  (let [result (.getClientProperty result-editor inspector-repl-result-key)]
    (if-not (and result (:editable result) (:source-start result) (:source-end result))
      (.setText inspector-status "result is inspect-only; no source range is available")
      (if-let [editor (active-editor tabs)]
        (let [replacement (.getText result-editor)
              start (:source-start result)
              end (:source-end result)
              source (.getText editor)]
          (try
            (let [candidate (inspector-validate-replacement! source start end replacement)]
              (cancel-live-auto-update! editor)
              (.putClientProperty editor live-auto-update-suppressed-key true)
              (try
                (.setText editor candidate)
                (.setCaretPosition editor (min start (count candidate)))
                (refresh-syntax-colors! editor)
                (finally
                  (.putClientProperty editor live-auto-update-suppressed-key false)
                  (cancel-live-auto-update! editor)))
              (live-queue-update! frame editor (or (.getClientProperty tabs "mescript.status")
                                                   inspector-status)
                                  (:audio-device @state)
                                  candidate)
              (.setText inspector-status "applied; queued for next loop")
              (.putClientProperty result-editor inspector-repl-result-key
                                  (assoc result
                                         :source-end (+ start (dec (count replacement)))
                                         :text replacement)))
            (catch Exception ex
              (.setText inspector-status (clean-error-message ex)))))
	        (.setText inspector-status "no active editor")))))

(defn inspector-play-result! [^JFrame frame ^JTextComponent result-editor ^JLabel inspector-status]
  (let [source (.getText result-editor)]
    (if (has-track-form? source)
      (live-update! frame result-editor inspector-status (:audio-device @state))
      (.setText inspector-status "play needs a track result: inspect :kick, kick, or a sample/d form"))))

(defn inspector-stop-result! [^JTextComponent result-editor ^JLabel inspector-status]
  (live-stop! result-editor inspector-status))

(def child-window-set-key "mescript.child-windows")

(defn child-window-set [^JFrame frame]
  (let [root (.getRootPane frame)]
    (or (.getClientProperty root child-window-set-key)
        (let [windows (atom #{})]
          (.putClientProperty root child-window-set-key windows)
          windows))))

(defn unregister-child-window! [^JFrame frame ^Window child]
  (when (and frame child)
    (when (instance? JFrame child)
      (.setAlwaysOnTop ^JFrame child false))
    (swap! (child-window-set frame) disj child)))

(defn register-child-window! [^JFrame frame ^Window child]
  (when (and frame child)
    (when (instance? JFrame child)
      (.setAlwaysOnTop ^JFrame child true))
    (swap! (child-window-set frame) conj child)
    (.addWindowListener child
                        (proxy [WindowAdapter] []
                          (windowClosed [_]
                            (unregister-child-window! frame child))))
    (.toFront child))
  child)

(defn dispose-child-windows! [^JFrame frame]
  (when frame
    (let [windows @(child-window-set frame)]
      (doseq [^Window child windows]
        (when (.isDisplayable child)
          (.dispose child)))
      (reset! (child-window-set frame) #{}))))

(defn close-main-window! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (when (close-all-tabs! frame tabs status)
    (dispose-child-windows! frame)
    (close-live-process!)
    (.dispose frame)))

(defn refresh-theme-selector-buttons! [buttons]
  (let [selected (or (:theme @state) (saved-theme-id))]
    (doseq [[id ^JButton button] buttons]
      (.setEnabled button (not= id selected))
      (.setText button (str (if (= id selected) "* " "  ")
                            (:label (theme-option id)))))))

(defn show-theme-selector! [^JFrame frame ^JLabel status]
  (let [root (.getRootPane frame)
        ^JFrame existing (.getClientProperty root theme-selector-frame-key)]
    (when (and existing (not (.isDisplayable existing)))
      (.putClientProperty root theme-selector-frame-key nil))
    (if (and existing (.isDisplayable existing))
      (do
        (.setVisible existing true)
        (.toFront existing)
        (.requestFocus existing))
      (let [selector-frame (JFrame. "Theme Selector")
            panel (JPanel.)
            buttons (atom {})]
        (.setName selector-frame "mescript-theme-selector-frame")
        (.setLayout panel (BoxLayout. panel BoxLayout/Y_AXIS))
        (.setBorder panel (BorderFactory/createEmptyBorder 8 8 8 8))
        (doseq [{:keys [id label]} theme-options]
          (let [button (JButton. label)]
            (.setName button (str "mescript-theme-" id))
            (.setAlignmentX button Component/LEFT_ALIGNMENT)
            (.setMaximumSize button (Dimension. Integer/MAX_VALUE 26))
            (swap! buttons assoc id button)
            (.addActionListener button
                                (reify ActionListener
                                  (actionPerformed [_ _]
                                    (let [result (apply-theme-id! id)]
                                      (if (:ok result)
                                        (refresh-theme-selector-buttons! @buttons)
                                        (set-status! status
                                                     (str "theme failed: " (:message result))))))))
            (.add panel button)
            (.add panel (Box/createVerticalStrut 4))))
        (refresh-theme-selector-buttons! @buttons)
        (.add (.getContentPane selector-frame) panel BorderLayout/CENTER)
        (.setSize selector-frame 220 420)
        (.setLocationRelativeTo selector-frame frame)
        (register-child-window! frame selector-frame)
        (.putClientProperty root theme-selector-frame-key selector-frame)
        (.addWindowListener selector-frame
                            (proxy [WindowAdapter] []
                              (windowClosed [_]
                                (.putClientProperty root theme-selector-frame-key nil))))
        (when-let [theme (:theme-data @state)]
          (style-component! selector-frame theme))
        (.setVisible selector-frame true)))))

(defn inspector-run-command! [^JTabbedPane tabs ^JTextComponent result-editor ^JTextField command-field ^JLabel inspector-status]
  (if-let [editor (active-editor tabs)]
    (let [command (.getText command-field)
          result (inspector-command-result (.getText editor) command)]
	      (.setText result-editor (:text result))
	      (.setCaretPosition result-editor 0)
	      (refresh-syntax-colors! result-editor)
	      (.putClientProperty result-editor inspector-repl-result-key result)
      (.setText inspector-status
                (if (:editable result)
                  "editable result"
                  "inspect-only result")))
    (.setText inspector-status "no active editor")))

(defn show-inspector-repl! [^JFrame frame ^JTabbedPane tabs ^JLabel status]
  (let [^JFrame existing (.getClientProperty (.getRootPane frame) inspector-repl-frame-key)]
    (when (and existing (not (.isDisplayable existing)))
      (.putClientProperty (.getRootPane frame) inspector-repl-frame-key nil))
    (if (and existing (.isDisplayable existing))
      (do
        (.setVisible existing true)
        (.toFront existing)
        (.requestFocus existing))
	      (let [repl-frame (JFrame. "Inspector")
	            result-editor (JTextPane.)
	            result-scroll (JScrollPane. result-editor)
	            command-field (JTextField.)
	            run-button (JButton. "Run")
	            play-button (JButton. "Play")
	            stop-button (JButton. "Stop")
	            apply-button (JButton. "Apply")
	            command-row (JPanel. (BorderLayout. 4 0))
	            button-row (JPanel.)
	            bottom-panel (JPanel. (BorderLayout.))
	            inspector-status (JLabel. "ready")]
      (.setName repl-frame "mescript-inspector-repl-frame")
      (.setName result-editor "mescript-inspector-result")
	      (.setName command-field "mescript-inspector-command")
	      (.setName run-button "mescript-inspector-run")
	      (.setName play-button "mescript-inspector-play")
	      (.setName stop-button "mescript-inspector-stop")
	      (.setName apply-button "mescript-inspector-apply")
	      (.setFont result-editor (Font. Font/MONOSPACED Font/PLAIN 13))
	      (.setEditable result-editor true)
	      (install-syntax-highlighter! result-editor)
	      (.setVerticalScrollBarPolicy result-scroll ScrollPaneConstants/VERTICAL_SCROLLBAR_ALWAYS)
	      (.setHorizontalScrollBarPolicy result-scroll ScrollPaneConstants/HORIZONTAL_SCROLLBAR_ALWAYS)
      (.add command-row (JLabel. "command:") BorderLayout/WEST)
      (.add command-row command-field BorderLayout/CENTER)
	      (.add button-row run-button)
	      (.add button-row play-button)
	      (.add button-row stop-button)
	      (.add button-row apply-button)
      (.add command-row button-row BorderLayout/EAST)
      (.setBorder command-row (BorderFactory/createEmptyBorder 4 4 4 4))
      (.setBorder inspector-status (BorderFactory/createEmptyBorder 2 6 4 6))
      (.addActionListener run-button
                          (reify ActionListener
                            (actionPerformed [_ _]
                              (inspector-run-command! tabs result-editor command-field inspector-status))))
      (.addActionListener command-field
                          (reify ActionListener
                            (actionPerformed [_ _]
                              (inspector-run-command! tabs result-editor command-field inspector-status))))
	      (.addActionListener apply-button
	                          (reify ActionListener
	                            (actionPerformed [_ _]
	                              (inspector-apply-result! frame tabs result-editor inspector-status))))
	      (.addActionListener play-button
	                          (reify ActionListener
	                            (actionPerformed [_ _]
	                              (inspector-play-result! frame result-editor inspector-status))))
	      (.addActionListener stop-button
	                          (reify ActionListener
	                            (actionPerformed [_ _]
	                              (inspector-stop-result! result-editor inspector-status))))
      (.setLayout (.getContentPane repl-frame) (BorderLayout.))
      (.add bottom-panel command-row BorderLayout/CENTER)
      (.add bottom-panel inspector-status BorderLayout/SOUTH)
	      (.add (.getContentPane repl-frame) result-scroll BorderLayout/CENTER)
      (.add (.getContentPane repl-frame) bottom-panel BorderLayout/SOUTH)
	      (.setSize repl-frame 520 320)
	      (.setLocationRelativeTo repl-frame frame)
	      (register-child-window! frame repl-frame)
	      (.putClientProperty (.getRootPane frame) inspector-repl-frame-key repl-frame)
	      (.addWindowListener repl-frame
	                          (proxy [WindowAdapter] []
                            (windowClosed [_]
                              (.putClientProperty (.getRootPane frame) inspector-repl-frame-key nil))))
      (.setVisible repl-frame true)
      (.requestFocusInWindow command-field)))))

(def about-text
  "MeScript v0.38\n28 June 2026\nJacob Pereira (jacob.m.pereira@gmail.com)\nquadracollision.com")

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
  (let [frame (JFrame. "MeScript")
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
    (.setPreferredSize tools (Dimension. (saved-tools-width) 480))
    (.setMinimumSize tools (Dimension. 96 0))
    (.setMaximumSize tools (Dimension. 420 32767))
    (set-insert-options! insert-form-combo "Oscillator")

    (letfn [(button [text f]
              (doto (JButton. text)
                (.setName (str "mescript-" (str/lower-case text) "-button"))
                (.addActionListener (reify ActionListener
                                      (actionPerformed [_ _] (f))))))
            (compact! [component height]
              (.setMaximumSize component (Dimension. Integer/MAX_VALUE height))
              (.setPreferredSize component (Dimension. (max 132 (.getWidth tools)) height))
              (.setMinimumSize component (Dimension. 80 height))
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
                (doto (menu-item "Close Tab" #(close-current-tab! frame tabs status))
                  (.setAccelerator close-current-tab-keystroke)))
	          (.add file-menu
	                (menu-item "Exit"
	                           #(close-main-window! frame tabs status)))
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
	          (.add preferences-menu
	                (menu-item "Theme Selector" #(show-theme-selector! frame status)))
          (.add preferences-menu
                (menu-item "Reset Window Layout"
	                           #(when-let [split (.getClientProperty (.getRootPane frame) "mescript.main-split")]
	                              (let [location (.getLocation frame)]
	                                (clear-layout-preferences!)
	                                (.setPreferredSize tools (Dimension. default-tools-width 480))
	                                (.setSize frame default-window-width default-window-height)
	                                (.setLocation frame location)
	                                (set-tools-panel-width! ^JSplitPane split default-tools-width))
	                              (save-tools-width! default-tools-width)
	                              (save-window-bounds! frame)
	                              (set-status! status "window layout: reset"))))
          (.add preferences-menu
	                (menu-item "Open Inspector" #(show-inspector-repl! frame tabs status)))
          (.add audio-menu devices-menu)
          (.add audio-menu (menu-item "Refresh Devices" #(refresh-devices!)))
	          (.add about-menu
	                (menu-item "Language Reference"
	                           #(register-child-window! frame (show-language-reference! frame))))
          (.add about-menu (menu-item "About MeScript" #(show-about! frame)))
          (.add menu-bar file-menu)
          (.add menu-bar preferences-menu)
          (.add menu-bar audio-menu)
          (.add menu-bar about-menu)
          (.setJMenuBar frame menu-bar)
          (refresh-devices!)))

      (let [editor (active-editor tabs)]
        (let [split (JSplitPane. JSplitPane/HORIZONTAL_SPLIT tabs tools)]
          (.setName split "mescript-main-split")
          (.setOneTouchExpandable split false)
          (.setContinuousLayout split true)
          (.setResizeWeight split 1.0)
          (.setDividerSize split 4)
          (.setBorder split nil)
          (.setMinimumSize tabs (Dimension. 280 0))
          (.addPropertyChangeListener
            split
            JSplitPane/DIVIDER_LOCATION_PROPERTY
            (proxy [java.beans.PropertyChangeListener] []
              (propertyChange [_]
                (let [width (.getWidth tools)]
                  (when (pos? width)
                    (save-tools-width! width))))))
          (.putClientProperty (.getRootPane frame) "mescript.main-split" split)
          (.putClientProperty (.getRootPane frame) "mescript.editor" editor)
          (.add (.getContentPane frame) split BorderLayout/CENTER))
        (.add (.getContentPane frame) status BorderLayout/SOUTH)))

    (.setDefaultCloseOperation frame JFrame/DO_NOTHING_ON_CLOSE)
    (.setName frame "mescript-workstation-frame")
    (.setName status "mescript-status")
    (.putClientProperty (.getRootPane frame) "mescript.editor-tabs" tabs)
    (.putClientProperty (.getRootPane frame) "mescript.status" status)
    (when-let [theme (:theme-data @state)]
      (style-component! frame theme))
    (install-app-shortcuts! frame tabs status)
	    (.addWindowListener frame
	                        (proxy [WindowAdapter] []
	                          (windowClosing [_]
	                            (close-main-window! frame tabs status))))
    (.pack frame)
	    (if-let [{:keys [x y width height]} (saved-window-bounds)]
	      (do
	        (.setSize frame width height)
	        (.setLocation frame x y))
	      (do
	        (.setSize frame default-window-width default-window-height)
	        (.setLocationRelativeTo frame nil)))
	    (.setVisible frame true)
	    (when-let [split (.getClientProperty (.getRootPane frame) "mescript.main-split")]
	      (set-tools-panel-width! ^JSplitPane split (saved-tools-width)))
	    (save-window-bounds! frame)
    (.addComponentListener frame
                           (proxy [ComponentAdapter] []
                             (componentMoved [_] (save-window-bounds! frame))
                             (componentResized [_] (save-window-bounds! frame))))
    frame))

(defn -main [& args]
  (let [file (some-> (first args) File.)]
    (SwingUtilities/invokeLater
      #(do
         (apply-saved-theme!)
         (build-ui file)))))

(defn no-gui-mode? []
  (or (System/getenv "GLITCHLISP_NO_GUI")
      (System/getProperty "glitchlisp.noGui")
      (GraphicsEnvironment/isHeadless)))

(when-not (no-gui-mode?)
  (apply -main *command-line-args*))
