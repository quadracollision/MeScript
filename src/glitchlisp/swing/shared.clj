(ns glitchlisp.swing.shared
  (:require [clojure.java.io :as io])
  (:import
    [java.io File]
    [javax.swing JComboBox JLabel]))

(defonce state
  (atom {:file nil
         :clip nil
         :rendering false
         :live-process nil
         :live-writer nil
         :live-device nil
         :live-ready false
         :live-last-error nil
         :live-audio-info nil
         :live-tracks nil
         :live-scenes nil
         :live-awaiting-update false
         :live-update-token 0
         :live-auto-edit-token 0
         :live-auto-last-error nil
         :live-highlight-step nil
         :live-highlight-scene nil
         :live-cycle nil
         :live-highlight-scheduled false
         :audio-device nil
         :remove-insert-comments true
         :remove-playback-highlighting false}))

(defn resource-slurp [path]
  (when-let [resource (io/resource path)]
    (slurp resource)))

(defn file-or-resource-slurp [path]
  (if (.exists (File. path))
    (slurp path)
    (resource-slurp path)))

(defn app-dir []
  (File. (or (System/getProperty "mescript.app.dir")
             (System/getProperty "user.dir"))))

(defn child-file [^File parent child]
  (File. parent child))

(defn set-status! [^JLabel status text]
  (.setText status text))

(def default-audio-device-label "Default output")

(defn live-running? [snapshot]
  (boolean (or (:live-awaiting-update snapshot)
               (:live-ready snapshot)
               (:live-process snapshot))))

(defn live-status-lines [snapshot]
  (let [running? (live-running? snapshot)]
    [(str "Device: " (or (:live-audio-info snapshot)
                         (:audio-device snapshot)
                         default-audio-device-label))
     (str "State: " (if running? "running" "stopped"))
     (str "Tracks: " (or (:live-tracks snapshot) "-"))
     (str "Scenes: " (or (:live-scenes snapshot) "-"))
     (str "Scene: " (if running? (or (:live-highlight-scene snapshot) "-") "-"))
     (str "Cycle: " (if running? (or (:live-cycle snapshot) "-") "-"))
     (str "Step: " (if running? (or (:live-highlight-step snapshot) "-") "-"))
     (str "Error: " (or (:live-last-error snapshot) "-"))]))

(defn selected-audio-device [^JComboBox combo]
  (let [selected (.getSelectedItem combo)]
    (when (and selected
               (not= default-audio-device-label (str selected)))
      (str selected))))

(defn set-combo-items! [^JComboBox combo values]
  (let [selected (.getSelectedItem combo)
        values (vec values)]
    (.removeAllItems combo)
    (doseq [value values]
      (.addItem combo value))
    (when (some #(= selected %) values)
      (.setSelectedItem combo selected))))
