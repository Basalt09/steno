import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

type Status = "idle" | "recording" | "transcribing" | "done" | "error";

interface TranscriptEntry {
  id: number;
  text: string;
  timestamp: string;
}

export default function App() {
  const [status, setStatus] = useState<Status>("idle");
  const [history, setHistory] = useState<TranscriptEntry[]>([]);
  const [errorMsg, setErrorMsg] = useState("");
  const [copiedId, setCopiedId] = useState<number | null>(null);

  const handleCopy = async (entry: TranscriptEntry) => {
    try {
      await navigator.clipboard.writeText(entry.text);
      setCopiedId(entry.id);
      setTimeout(
        () => setCopiedId((current) => (current === entry.id ? null : current)),
        1200,
      );
    } catch (e) {
      console.error("Clipboard write failed:", e);
    }
  };

  useEffect(() => {
    // Listen for recording start
    const unlistenStart = listen<void>("recording-start", () => {
      setStatus("recording");
    });

    // Listen for transcribing state
    const unlistenTranscribing = listen<void>("transcribing", () => {
      setStatus("transcribing");
    });

    // Listen for completed transcription
    const unlistenResult = listen<string>("transcription", (event) => {
      const text = event.payload;
      setStatus("done");
      setHistory((prev) => [
        {
          id: Date.now(),
          text,
          timestamp: new Date().toLocaleTimeString(),
        },
        ...prev.slice(0, 19), // keep last 20
      ]);
      setTimeout(() => setStatus("idle"), 1500);
    });

    // Listen for errors
    const unlistenError = listen<string>("transcription-error", (event) => {
      setStatus("error");
      setErrorMsg(event.payload);
      setTimeout(() => setStatus("idle"), 3000);
    });

    return () => {
      unlistenStart.then((f) => f());
      unlistenTranscribing.then((f) => f());
      unlistenResult.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  const statusConfig: Record<Status, { label: string; color: string; pulse: boolean }> = {
    idle:         { label: "Ready",        color: "#4ade80", pulse: false },
    recording:    { label: "Recording…",   color: "#f87171", pulse: true  },
    transcribing: { label: "Processing…",  color: "#facc15", pulse: true  },
    done:         { label: "Typed ✓",      color: "#60a5fa", pulse: false },
    error:        { label: "Error",        color: "#f87171", pulse: false },
  };

  const { label, color, pulse } = statusConfig[status];

  return (
    <div className="app">
      <header className="header">
        <div className="title-row">
          <span className="icon">🎙</span>
          <h1>Steno</h1>
        </div>
        <p className="hotkey-hint">Hold <kbd>Shift</kbd>+<kbd>Space</kbd> to speak</p>
      </header>

      <div className="status-card">
        <div className={`indicator ${pulse ? "pulse" : ""}`} style={{ background: color }} />
        <span className="status-label" style={{ color }}>{label}</span>
        {status === "error" && <p className="error-detail">{errorMsg}</p>}
      </div>

      <div className="history-section">
        <h2>Recent</h2>
        {history.length === 0 ? (
          <p className="empty-state">Nothing yet — hold the hotkey and speak</p>
        ) : (
          <ul className="history-list">
            {history.map((entry) => (
              <li key={entry.id} className="history-item">
                <span className="history-text">{entry.text}</span>
                <span className="history-time">{entry.timestamp}</span>
                <button
                  type="button"
                  className={`copy-btn ${copiedId === entry.id ? "copied" : ""}`}
                  onClick={() => handleCopy(entry)}
                  aria-label={copiedId === entry.id ? "Copied" : "Copy text"}
                  title={copiedId === entry.id ? "Copied!" : "Copy"}
                >
                  {copiedId === entry.id ? (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                    </svg>
                  )}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
