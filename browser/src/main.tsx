import { Component, StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { GameClient } from "./controller";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Player root unavailable.");
// One transport owner per page, independent of React rendering and effect replay.
const client = new GameClient();
void client.boot();

class PlayerBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() {
    return this.state.failed ? <main className="recovery-panel"><h1>Player unavailable</h1><p>Reload to recover your local session.</p><button onClick={() => location.reload()}>Reload player</button></main> : this.props.children;
  }
}

createRoot(root).render(<StrictMode><PlayerBoundary><App client={client} /></PlayerBoundary></StrictMode>);
