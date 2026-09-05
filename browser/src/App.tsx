import { useCallback, useMemo, useState, useSyncExternalStore, type FormEvent } from "react";
import type { GameClient } from "./controller";
import type { ActionView, ClientState, StartOptions, StartRecipe } from "./types";
import { actionLabel, filterActions, publicMessage } from "./presentation";

const PAGE_SIZE = 12;

type PlayerClient = Pick<GameClient, "subscribe" | "getSnapshot" | "close" | "save" | "resume" | "acknowledgeRestart" | "retry" | "refresh" | "start" | "newGame" | "act">;

export function App({ client }: { client: PlayerClient }) {
  const subscribe = useCallback((listener: () => void) => client.subscribe(listener), [client]);
  const snapshot = useCallback(() => client.getSnapshot(), [client]);
  const state = useSyncExternalStore(subscribe, snapshot, snapshot);
  const [fileMessage, setFileMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const busy = saving || state.phase === "working" || state.phase === "booting";

  async function downloadSave(closeFirst = false) {
    setSaving(true);
    setFileMessage(null);
    try {
      if (closeFirst) {
        await client.close();
        if (client.getSnapshot().phase !== "closed") return;
      }
      const save = await client.save();
      const url = URL.createObjectURL(new Blob([save], { type: "application/json" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = "adventure-forge.trace.json";
      document.body.append(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      setFileMessage("Save download requested. Keep the file to resume this game.");
    } catch {
      setFileMessage("The save could not be downloaded. Your game has not been discarded.");
    } finally {
      setSaving(false);
    }
  }

  async function importSave(file: File | undefined) {
    if (!file) return;
    setSaving(true);
    setFileMessage(null);
    try {
      if (file.size > 256 * 1024) {
        setFileMessage("This save exceeds the local server's 256 KiB limit.");
        return;
      }
      // Save text is opaque: do not round its integers or rewrite its JSON.
      const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true })
        .decode(await file.arrayBuffer());
      await client.resume(text);
    } catch {
      setFileMessage("This file could not be read as a save. Choose an exported game file.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="player-shell">
      <a className="skip-link" href="#main">Skip to game</a>
      <header className="site-header">
        <a href="/" className="wordmark" aria-label="Adventure Forge home">
          <span className="brand-mark" aria-hidden="true">AF</span>
          <span>Adventure Forge</span>
        </a>
        <span className="local-badge"><span aria-hidden="true">●</span> Local play</span>
      </header>
      <main id="main">
        <div className="chapter-line"><span>Veyra Basin</span><span className="rule" /><span>The Split Tide</span></div>
        {state.storageWarning && <div className="notice warning" role="alert">{publicMessage(state.storageWarning)}</div>}
        {(state.message || fileMessage) && (
          <div className="notice" role="status">
            {state.message && <p>{publicMessage(state.message)}</p>}
            {fileMessage && <p>{fileMessage}</p>}
          </div>
        )}
        {state.phase === "booting" && <div className="empty-state" role="status">Opening the world…</div>}
        {state.phase === "restarted" && (
          <section className="recovery-panel">
            <p className="eyebrow">Session recovery</p>
            <h1>Confirm the local session</h1>
            <p>Continue to check for an active game. If none remains, resume a downloaded save or start again.</p>
            <button onClick={() => void client.acknowledgeRestart()}>Continue recovery</button>
          </section>
        )}
        {state.phase === "uncertain" && (
          <section className="recovery-panel" aria-labelledby="retry-title">
            <p className="eyebrow">Connection interrupted</p>
            <h2 id="retry-title">Keep the same request.</h2>
            <p>Its result is not confirmed. Retry safely before choosing another action.</p>
            <button onClick={() => void client.retry()}>Retry request</button>
          </section>
        )}
        {state.phase === "error" && (
          <section className="recovery-panel">
            <h2>The player needs to reconnect.</h2>
            <button onClick={() => void client.refresh()}>Reconnect</button>
          </section>
        )}
        {state.options && !state.session && (state.phase === "start" || state.phase === "working") && (
          <StartScreen options={state.options} disabled={busy} onStart={(start) => void client.start(start)} onImport={importSave} />
        )}
        {state.session && (
          <>
            <div className="session-toolbar">
              <span className="session-status" role="status">
                {state.phase === "closed" ? "Session closed · your save is still available" : busy ? "Confirming…" : "Your journey"}
              </span>
              <div className="toolbar-buttons">
                <button className="quiet-button" disabled={busy || !["ready", "closed"].includes(state.phase)} onClick={() => void downloadSave()}>Download save</button>
                {state.phase === "closed" ? (
                  <button className="quiet-button" disabled={busy} onClick={() => { setFileMessage(null); void client.newGame(); }}>New character</button>
                ) : (
                  <button className="quiet-button" disabled={busy || state.phase !== "ready"} onClick={() => void downloadSave(true)}>Save and close</button>
                )}
              </div>
            </div>
            <GameScreen state={state} disabled={busy || state.phase !== "ready" || !state.catalogComplete} onAct={async (action) => {
              setFileMessage(null);
              await client.act(action);
              if (client.getSnapshot().phase === "ready") document.getElementById("location-title")?.focus();
            }} />
          </>
        )}
        {!state.session && state.phase === "closed" && (
          <section className="recovery-panel">
            <h1>Session closed</h1>
            <p>You can download its save or begin a new character.</p>
            <div className="toolbar-buttons">
              <button disabled={busy} onClick={() => void downloadSave()}>Download save</button>
              <button className="quiet-button" disabled={busy} onClick={() => void client.newGame()}>New character</button>
            </div>
          </section>
        )}
      </main>
      <footer>Save files belong to this exact game build. Download one before stopping the local server.</footer>
    </div>
  );
}

function StartScreen({ options, disabled, onStart, onImport }: {
  options: StartOptions;
  disabled: boolean;
  onStart: (start: StartRecipe) => void;
  onImport: (file: File | undefined) => Promise<void>;
}) {
  const [custom, setCustom] = useState(false);
  const [seed, setSeed] = useState("71");
  const [name, setName] = useState("");
  const [choices, setChoices] = useState<Record<string, string>>({});

  function create(event: FormEvent) {
    event.preventDefault();
    onStart({ kind: "custom", seed, selection: { name, choices: options.creation_slots.map((slot) => ({
      slot_id: slot.id, choice_id: choices[slot.id] ?? slot.choices[0]?.id ?? "",
    })) } });
  }

  return (
    <section className="start-layout">
      <div className="start-intro">
        <p className="eyebrow">An adventure in one world</p>
        <h1>Who enters<br /><em>the tide?</em></h1>
        <p className="lede">Choose a character and enter the world.</p>
        <div className="resume-box">
          <h2>Already on a journey?</h2>
          <p>Resume an exported save from this game build.</p>
          <label className={`file-button ${disabled ? "disabled" : ""}`}>
            Resume a save
            <input aria-label="Resume a save" type="file" accept=".json,application/json" disabled={disabled} onChange={(event) => {
              void onImport(event.target.files?.[0]);
              event.target.value = "";
            }} />
          </label>
        </div>
        <details className="seed-settings">
          <summary>World seed</summary>
          <label htmlFor="world-seed">Seed</label>
          <input id="world-seed" type="text" inputMode="numeric" value={seed} disabled={disabled} onChange={(event) => setSeed(event.target.value)} autoComplete="off" spellCheck={false} />
          <p>The same choices and seed replay the same history.</p>
        </details>
      </div>
      <div className="character-options">
        <div className="section-heading"><h2>Choose your beginning</h2><span>{custom ? "Your character" : "Authored characters"}</span></div>
        <div className="preset-grid">
          {options.presets.map((preset, index) => (
            <article className="preset-card" key={preset.id}>
              <span className="card-number" aria-hidden="true">0{index + 1}</span>
              <h3>{preset.display_name}</h3>
              <p>{preset.summary}</p>
              <button disabled={disabled} onClick={() => onStart({ kind: "preset", character_preset_id: preset.id, seed })}>Start as {preset.display_name}</button>
            </article>
          ))}
        </div>
        <section className="custom-section">
          <button className="custom-toggle" aria-expanded={custom} aria-controls="custom-creator" disabled={disabled} onClick={() => setCustom(!custom)}>
            <span>Create your own character</span><span aria-hidden="true">{custom ? "−" : "+"}</span>
          </button>
          {custom && (
            <form id="custom-creator" onSubmit={create}>
              <label htmlFor="character-name">Name</label>
              <input id="character-name" value={name} onChange={(event) => setName(event.target.value)} required disabled={disabled} autoComplete="off" />
              <div className="creation-grid">
                {options.creation_slots.map((slot) => {
                  const value = choices[slot.id] ?? slot.choices[0]?.id ?? "";
                  const choice = slot.choices.find((entry) => entry.id === value);
                  return (
                    <div className="creation-field" key={slot.id}>
                      <label htmlFor={`slot-${slot.id}`}>{slot.display_name}</label>
                      <select id={`slot-${slot.id}`} value={value} disabled={disabled} aria-describedby={`description-${slot.id}`} onChange={(event) => setChoices({ ...choices, [slot.id]: event.target.value })}>
                        {slot.choices.map((entry) => <option key={entry.id} value={entry.id}>{entry.display_name}</option>)}
                      </select>
                      <p id={`description-${slot.id}`}>{choice?.summary}</p>
                    </div>
                  );
                })}
              </div>
              <button type="submit" disabled={disabled}>Begin your journey</button>
            </form>
          )}
        </section>
      </div>
    </section>
  );
}

function GameScreen({ state, disabled, onAct }: { state: ClientState; disabled: boolean; onAct: (action: ActionView) => void }) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");
  const [page, setPage] = useState(0);
  const categories = useMemo(() => [...new Set(state.actions.map((action) => action.category))], [state.actions]);
  const activeCategory = categories.includes(category) ? category : "";
  const filtered = useMemo(() => filterActions(state.actions, query, activeCategory), [state.actions, query, activeCategory]);
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const currentPage = Math.min(page, pages - 1);
  const visible = filtered.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
  const observation = state.session?.view.observation;
  if (!observation) return null;

  return (
    <div className="game-layout">
      <section className="scene-panel" aria-labelledby="location-title">
        <div className="scene-topline"><span className="eyebrow">Here & now</span><span className="time-badge">Tide step {observation.world_time}</span></div>
        <div className="narrative" role="status" aria-live="polite" aria-atomic="true">
          <h1 id="location-title" tabIndex={-1}>{observation.title}</h1>
          <p className="scene-text">{observation.text}</p>
        </div>
        {observation.upcoming_events.length > 0 && (
          <div className="tide-events">
            {observation.upcoming_events.map((event, index) => <p key={index}><span>{event.label}</span><strong>{event.remaining_ticks} steps away</strong></p>)}
          </div>
        )}
        <section className="supplies" aria-labelledby="supplies-title">
          <h2 id="supplies-title">Your supplies</h2>
          <dl>
            {observation.supplies.resources.map((resource) => <div key={resource.id}><dt>{resource.name}</dt><dd>{resource.amount}</dd></div>)}
            {observation.supplies.items.map((item) => <div key={item.id}><dt>{item.name}</dt><dd>{item.count}</dd></div>)}
          </dl>
          {!observation.supplies.resources.length && !observation.supplies.items.length && <p>No supplies listed.</p>}
        </section>
      </section>
      <section className="actions-panel" aria-labelledby="actions-title" aria-busy={!state.catalogComplete}>
        <div className="section-heading"><h2 id="actions-title">What do you do?</h2><span>{observation.action_count} choices</span></div>
        <div className="action-filters">
          <label className="search-field">Search all choices<input type="search" placeholder="Find an action or consequence" value={query} onChange={(event) => { setQuery(event.target.value); setPage(0); }} /></label>
          <label>Category<select value={activeCategory} onChange={(event) => { setCategory(event.target.value); setPage(0); }}><option value="">All categories</option>{categories.map((entry) => <option key={entry} value={entry}>{entry}</option>)}</select></label>
        </div>
        {!state.catalogComplete && <p role="status">Loading the complete action catalog…</p>}
        <div className="action-grid">
          {visible.map((action) => (
            <button className="action-card" key={action.action_id} disabled={disabled} onClick={() => onAct(action)}>
              <span className="action-meta"><span>{action.category}</span><span>{action.time_cost.minimum_ticks === action.time_cost.maximum_ticks ? action.time_cost.minimum_ticks : `${action.time_cost.minimum_ticks}–${action.time_cost.maximum_ticks}`} tide {action.time_cost.minimum_ticks === "1" && action.time_cost.maximum_ticks === "1" ? "step" : "steps"}</span></span>
              <span className="action-label">{actionLabel(action)}<span className="action-arrow" aria-hidden="true">↗</span></span>
              {action.consequence_preview && <span className="action-preview">{action.consequence_preview}</span>}
            </button>
          ))}
        </div>
        {state.catalogComplete && !visible.length && <div className="empty-state">{state.actions.length ? "No choices match these filters." : "No actions are available."}</div>}
        {(query || activeCategory) && <button className="text-button" onClick={() => { setQuery(""); setCategory(""); setPage(0); }}>Clear filters</button>}
        {pages > 1 && <nav className="pagination" aria-label="Action pages"><button className="quiet-button" disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)}>Previous</button><span>Page {currentPage + 1} of {pages}</span><button className="quiet-button" disabled={currentPage === pages - 1} onClick={() => setPage(currentPage + 1)}>Next</button></nav>}
      </section>
    </div>
  );
}
