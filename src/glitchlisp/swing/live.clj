(ns glitchlisp.swing.live
  (:require [clojure.string :as str]
            [glitchlisp.swing.editor :as editor]
            [glitchlisp.swing.shared :as shared])
  (:import
    [java.awt GraphicsEnvironment]
    [java.io BufferedReader InputStreamReader OutputStreamWriter]
    [javax.swing JFrame JLabel JOptionPane SwingUtilities]
    [javax.swing.text JTextComponent]))

(def live-end-marker "__GLITCHLISP_END__")
(def live-update-timeout-ms 3000)

(defn live-process-running? []
  (when-let [^Process process (:live-process @shared/state)]
    (.isAlive process)))

(defn close-live-process! []
  (when-let [writer (:live-writer @shared/state)]
    (try
      (.write writer "QUIT\n")
      (.flush writer)
      (catch Exception _)))
  (when-let [^Process process (:live-process @shared/state)]
    (when (.isAlive process)
      (.destroy process)))
  (swap! shared/state
         (fn [current]
           (assoc current
                  :live-process nil
                  :live-writer nil
                  :live-device nil
                  :live-ready false
                  :live-audio-info nil
                  :live-tracks nil
                  :live-scenes nil
                  :live-awaiting-update false
                  :live-update-token (inc (long (or (:live-update-token current) 0)))
                  :live-highlight-step nil
                  :live-highlight-scene nil
                  :live-cycle nil
                  :live-highlight-scheduled false))))

(defn begin-live-update! []
  (let [token (atom nil)]
    (swap! shared/state
           (fn [current]
             (let [next-token (inc (long (or (:live-update-token current) 0)))]
               (reset! token next-token)
               (assoc current
                      :live-awaiting-update true
                      :live-update-token next-token
                      :live-last-error nil))))
    @token))

(defn complete-live-update! []
  (swap! shared/state assoc :live-awaiting-update false))

(defn live-summary-with-step [summary scene cycle]
  (when summary
    (cond-> summary
      scene
      (str/replace #"scene=[^\s]+" (str "scene=:" scene))

      cycle
      (str/replace #"cycle=[^\s]+" (str "cycle=" cycle)))))

(defn live-step-status-text []
  (when-let [summary (:live-status-summary @shared/state)]
    (str "live " summary)))

(defn update-live-step-state!
  ([step scene]
   (update-live-step-state! step scene nil))
  ([step scene cycle]
   (swap! shared/state
          (fn [current]
            (cond-> (assoc current
                           :live-highlight-step step
                           :live-highlight-scene scene)
              cycle
              (assoc :live-cycle cycle)

              (or scene cycle)
              (update :live-status-summary live-summary-with-step scene cycle)

              (and (nil? cycle) (not= scene (:live-highlight-scene current)))
              (assoc :live-cycle nil))))))

(defn queue-playback-highlight! [^JTextComponent editor-pane received-ns]
  (if (:remove-playback-highlighting @shared/state)
    (editor/clear-live-step-highlight! editor-pane)
    (if-let [highlight-fn (:live-highlight-fn @shared/state)]
      (highlight-fn (:live-highlight-step @shared/state)
                    (:live-highlight-scene @shared/state)
                    received-ns)
      (editor/queue-current-live-step-highlight! editor-pane received-ns))))

(defn parse-step-line [line]
  (when (str/starts-with? line "STEP ")
    (let [length (count line)]
      (let [step-start (loop [idx 5]
                         (if (and (< idx length)
                                  (Character/isWhitespace ^char (.charAt line idx)))
                           (recur (inc idx))
                           idx))]
        (when (and (< step-start length)
                   (Character/isDigit ^char (.charAt line step-start)))
          (let [step-end (loop [idx (inc step-start)]
                           (if (and (< idx length)
                                    (Character/isDigit ^char (.charAt line idx)))
                             (recur (inc idx))
                             idx))]
            (try
              (let [step (Long/parseLong (subs line step-start step-end))]
                (if (= step-end length)
                  [step nil nil]
                  (let [scene-start (loop [idx step-end]
                                      (if (and (< idx length)
                                               (Character/isWhitespace ^char (.charAt line idx)))
                                        (recur (inc idx))
                                        idx))]
                    (when (and (< scene-start length)
                               (= (.charAt line scene-start) \:)
                               (< (inc scene-start) length))
                      (let [scene-end (loop [idx (inc scene-start)]
                                        (if (and (< idx length)
                                                 (not (Character/isWhitespace ^char (.charAt line idx))))
                                          (recur (inc idx))
                                          idx))
                            scene (subs line (inc scene-start) scene-end)
                            rest-start (loop [idx scene-end]
                                         (if (and (< idx length)
                                                  (Character/isWhitespace ^char (.charAt line idx)))
                                           (recur (inc idx))
                                           idx))]
                        (cond
                          (str/blank? scene) nil
                          (= rest-start length) [step scene nil]
                          :else (when-let [[_ cycle] (re-matches #"cycle=([^\s]+)" (subs line rest-start))]
                                  [step scene cycle])))))))
              (catch Exception _ nil))))))))

(defn expire-live-update! [^JLabel status token]
  (let [message (atom nil)]
    (swap! shared/state
           (fn [current]
             (if (and (:live-awaiting-update current)
                      (= token (:live-update-token current)))
               (let [process ^Process (:live-process current)
                     text (if (and process (.isAlive process))
                            "live update timed out; no response from live engine"
                            "live engine stopped before responding")]
                 (reset! message text)
                 (assoc current
                        :live-awaiting-update false
                        :live-last-error text))
               current)))
    (when @message
      (shared/set-status! status @message))))

(defn schedule-live-update-timeout! [^JLabel status token]
  (future
    (Thread/sleep live-update-timeout-ms)
    (SwingUtilities/invokeLater #(expire-live-update! status token))))

(defn handle-live-line! [^JTextComponent editor-pane ^JLabel status line]
  (cond
    (str/starts-with? line "STEP ")
    (when-let [[step scene cycle] (parse-step-line line)]
      (let [received-ns (System/nanoTime)]
      (update-live-step-state! step scene cycle)
      (when (:live-awaiting-update @shared/state)
        (complete-live-update!)
        (shared/set-status! status "live running"))
      (when-let [text (live-step-status-text)]
        (shared/set-status! status text))
        (queue-playback-highlight! editor-pane received-ns)))

    (str/starts-with? line "AUDIO ")
    (let [info (subs line 6)]
      (swap! shared/state assoc :live-audio-info info :live-last-error nil)
      (shared/set-status! status (str "live audio: " info)))

    (= line "STOPPED")
    (do
      (swap! shared/state assoc
             :live-highlight-step nil
             :live-highlight-scene nil
             :live-cycle nil
             :live-highlight-scheduled false
             :live-last-error nil)
      (editor/clear-live-step-highlight! editor-pane)
      (shared/set-status! status "live stopped"))

    (= line "READY")
    (do
      (swap! shared/state assoc :live-ready true :live-last-error nil)
      (if-let [info (:live-audio-info @shared/state)]
        (shared/set-status! status (str "live engine ready: " info))
        (shared/set-status! status "live engine ready")))

    (str/starts-with? line "OK ")
    (do
      (let [[_ tracks scenes] (re-find #"tracks=([0-9]+).*scenes=([0-9]+)" line)
            [_ scene] (re-find #"scene=([^\s]+)" line)
            [_ cycle] (re-find #"cycle=([^\s]+)" line)
            summary (subs line 3)
            scene (when (and scene (not= scene "-"))
                    (if (str/starts-with? scene ":") (subs scene 1) scene))
            cycle (when (and cycle (not= cycle "-")) cycle)]
        (swap! shared/state assoc
               :live-last-error nil
               :live-status-summary summary
               :live-tracks (when tracks (Long/parseLong tracks))
               :live-scenes (when scenes (Long/parseLong scenes))
               :live-highlight-scene scene
               :live-cycle cycle))
      (complete-live-update!)
      (shared/set-status! status (str "live " (subs line 3))))

    (str/starts-with? line "ERR ")
    (let [message (subs line 4)]
      (swap! shared/state assoc :live-last-error message)
      (complete-live-update!)
      (let [ex (editor/source-error-exception (.getText editor-pane) message)
            clean-message (editor/clean-error-message ex)]
        (editor/focus-source-error! editor-pane status ex)
        (shared/set-status! status (str "live error: " clean-message))))

    :else
    (when-not (str/blank? line)
      (swap! shared/state assoc :live-last-error line)
      (shared/set-status! status line))))

(declare clear-ended-live-process!)

(defn start-live-reader! [^JTextComponent editor-pane ^JLabel status ^Process process]
  (future
    (try
      (with-open [reader (BufferedReader. (InputStreamReader. (.getInputStream process)))]
        (doseq [line (line-seq reader)]
          (if (str/starts-with? line "STEP ")
            (when-let [[step scene cycle] (parse-step-line line)]
              (let [received-ns (System/nanoTime)]
              (update-live-step-state! step scene cycle)
              (when (:live-awaiting-update @shared/state)
                (complete-live-update!)
                (SwingUtilities/invokeLater #(shared/set-status! status "live running")))
              (when-let [text (live-step-status-text)]
                (SwingUtilities/invokeLater #(shared/set-status! status text)))
                (queue-playback-highlight! editor-pane received-ns)))
            (SwingUtilities/invokeLater #(handle-live-line! editor-pane status line)))))
      (catch Exception ex
        (SwingUtilities/invokeLater #(shared/set-status! status (str "live reader failed: " (.getMessage ex)))))
      (finally
        (when-not (.isAlive process)
          (clear-ended-live-process! editor-pane process))))))

(defn clear-ended-live-process! [^JTextComponent editor-pane ^Process process]
  (let [cleared? (atom false)]
    (swap! shared/state
           (fn [current]
             (if (= process (:live-process current))
               (do
                 (reset! cleared? true)
                 (assoc current
                        :live-process nil
                        :live-writer nil
                        :live-device nil
                        :live-ready false
                        :live-audio-info nil
                        :live-tracks nil
                        :live-scenes nil
                        :live-awaiting-update false
                        :live-update-token (inc (long (or (:live-update-token current) 0)))
                        :live-highlight-step nil
                        :live-highlight-scene nil
                        :live-cycle nil
                        :live-highlight-scheduled false))
               current)))
    (when @cleared?
      (SwingUtilities/invokeLater #(editor/clear-live-step-highlight! editor-pane)))))

(defn wait-live-ready! [^Process process]
  (let [deadline (+ (System/currentTimeMillis) 3000)]
    (loop []
      (cond
        (:live-ready @shared/state)
        true

        (not (.isAlive process))
        (throw (ex-info (str "live engine failed to start"
                             (when-let [error (:live-last-error @shared/state)]
                               (str ": " error)))
                        {}))

        (> (System/currentTimeMillis) deadline)
        (throw (ex-info (str "live engine did not become ready"
                             (when-let [error (:live-last-error @shared/state)]
                               (str ": " error)))
                        {}))

        :else
        (do
          (Thread/sleep 20)
          (recur))))))

(defn ensure-live-process! [^JTextComponent editor-pane ^JLabel status device ensure-renderer!]
  (let [renderer (ensure-renderer! status)
        _ (when (live-process-running?)
            (close-live-process!))
        args (cond-> [renderer "gui-live"]
               device (conj "--device" device))
        process (.start (doto (ProcessBuilder. ^java.util.List args)
                          (.redirectErrorStream true)))
        writer (OutputStreamWriter. (.getOutputStream process))]
    (swap! shared/state
           (fn [current]
             (assoc current
                    :live-process process
                    :live-writer writer
                    :live-device device
                    :live-ready false
                    :live-last-error nil
                    :live-audio-info nil
                    :live-tracks nil
                    :live-scenes nil
                    :live-awaiting-update false
                    :live-update-token (inc (long (or (:live-update-token current) 0)))
                    :live-highlight-step nil
                    :live-highlight-scene nil
                    :live-cycle nil
                    :live-highlight-scheduled false)))
    (editor/clear-live-step-highlight! editor-pane)
    (start-live-reader! editor-pane status process)
    (wait-live-ready! process)
    writer))

(defn send-live-command! [command]
  (when-let [writer (:live-writer @shared/state)]
    (.write writer (str command "\n"))
    (.flush writer)))

(defn send-compiled-live-update!
  [^JTextComponent editor-pane ^JLabel status compiled waiting-status]
  (when-let [writer (:live-writer @shared/state)]
    (when (live-process-running?)
      (let [token (begin-live-update!)]
        (schedule-live-update-timeout! status token))
      (SwingUtilities/invokeLater #(shared/set-status! status waiting-status))
      (locking writer
        (.write writer "EVAL\n")
        (.write writer compiled)
        (when-not (str/ends-with? compiled "\n")
          (.write writer "\n"))
        (.write writer live-end-marker)
        (.write writer "\n")
        (.flush writer))
      true)))

(defn live-update!
  ([^JFrame frame ^JTextComponent editor-pane ^JLabel status device ensure-renderer! preview-source require-playback-form! compile-source]
   (live-update! frame editor-pane status device ensure-renderer! preview-source require-playback-form! compile-source nil))
  ([^JFrame frame ^JTextComponent editor-pane ^JLabel status device ensure-renderer! preview-source require-playback-form! compile-source highlight-fn]
  (future
    (try
      (swap! shared/state assoc :live-highlight-fn highlight-fn)
      (swap! shared/state assoc :live-awaiting-update false)
      (SwingUtilities/invokeLater #(shared/set-status! status "live compiling..."))
      (let [source (.getText editor-pane)
            _ (editor/validate-delimiters! source)
            preview (preview-source source)
            _ (require-playback-form! preview)
            compiled (compile-source preview)
            writer (ensure-live-process! editor-pane status device ensure-renderer!)]
        (send-compiled-live-update! editor-pane status compiled "waiting for live engine..."))
      (catch Exception ex
        (SwingUtilities/invokeLater
          #(do
             (editor/report-source-error! editor-pane status ex)
             (when-not (GraphicsEnvironment/isHeadless)
               (JOptionPane/showMessageDialog frame (editor/clean-error-message ex) "Live update failed" JOptionPane/ERROR_MESSAGE)))))))))

(defn live-stop! [^JTextComponent editor-pane ^JLabel status]
  (swap! shared/state assoc :live-awaiting-update false)
  (send-live-command! "STOP")
  (close-live-process!)
  (editor/clear-live-step-highlight! editor-pane)
  (shared/set-status! status "live stopped"))
