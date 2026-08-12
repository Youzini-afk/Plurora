const CAP = "plurora/modular-simulation";
const PROVIDER = "plurora/modular-simulation";

function outputOf(value) {
  return value && typeof value === "object" && "output" in value ? value.output : value;
}

async function invoke(name, input) {
  if (!window.pluroraHost?.callRpc) {
    throw new Error("Host capability bridge is unavailable");
  }
  return outputOf(await window.pluroraHost.callRpc("capability.invoke", {
    capability_id: CAP + "/" + name,
    provider_package_id: PROVIDER,
    input,
  }));
}

function cellLabel(cell) {
  const icon = cell.structure === "farm" ? "F" : cell.structure === "generator" ? "E" : cell.structure === "habitat" ? "H" : "·";
  return icon + (cell.workers ? String(cell.workers) : "");
}

export function mountSurface(root) {
  let disposed = false;
  let state = null;
  let notice = "Starting deterministic simulation…";

  root.innerHTML = `
    <style>
      .mod-sim { color:#eef5ff; background:linear-gradient(145deg,#101929,#17253c); min-height:100%; padding:18px; font:14px/1.45 system-ui,sans-serif; box-sizing:border-box }
      .mod-sim h1 { margin:0 0 4px; font-size:22px }
      .mod-sim .muted { color:#9eb1ca }
      .mod-sim .toolbar { display:flex; flex-wrap:wrap; gap:8px; margin:14px 0 }
      .mod-sim button { border:1px solid #537099; border-radius:8px; color:#eef5ff; background:#203756; padding:8px 11px; cursor:pointer }
      .mod-sim button:focus-visible { outline:3px solid #74c0fc; outline-offset:2px }
      .mod-sim .layout { display:grid; grid-template-columns:minmax(230px,1fr) minmax(230px,.9fr); gap:16px }
      .mod-sim .board { display:grid; grid-template-columns:repeat(5,minmax(38px,1fr)); gap:6px }
      .mod-sim .cell { aspect-ratio:1; display:grid; place-items:center; border:1px solid #3e587a; border-radius:7px; background:#142238; font-weight:700 }
      .mod-sim pre { white-space:pre-wrap; overflow:auto; max-height:360px; padding:12px; border-radius:8px; background:#0b1422 }
      .mod-sim .notice { min-height:22px; color:#a9d8ff }
      @media (max-width:720px){ .mod-sim .layout{grid-template-columns:1fr} }
    </style>
    <section class="mod-sim" aria-live="polite">
      <h1>Modular Simulation</h1>
      <div class="muted">A 5×5 rule-driven colony. State stays explicit and portable.</div>
      <div class="toolbar">
        <button data-action="build-farm">Build farm</button>
        <button data-action="build-generator">Build generator</button>
        <button data-action="advance">Advance turn</button>
        <button data-action="ai">Ask optional AI</button>
        <button data-action="save">Export save</button>
      </div>
      <div class="notice"></div>
      <div class="layout">
        <div>
          <div class="summary"></div>
          <div class="board" role="grid" aria-label="Simulation board"></div>
        </div>
        <pre class="inspector" aria-label="Simulation state inspector"></pre>
      </div>
    </section>`;

  const noticeNode = root.querySelector(".notice");
  const boardNode = root.querySelector(".board");
  const summaryNode = root.querySelector(".summary");
  const inspectorNode = root.querySelector(".inspector");

  function render() {
    if (disposed) return;
    noticeNode.textContent = notice;
    if (!state) return;
    summaryNode.textContent = `Turn ${state.turn} · Population ${state.population} · Energy ${state.resources.energy} · Food ${state.resources.food} · Materials ${state.resources.materials}`;
    boardNode.replaceChildren(...state.cells.map((cell) => {
      const element = document.createElement("div");
      element.className = "cell";
      element.setAttribute("role", "gridcell");
      element.title = `${cell.x},${cell.y}: ${cell.structure || "empty"}, workers ${cell.workers}`;
      element.textContent = cellLabel(cell);
      return element;
    }));
    inspectorNode.textContent = JSON.stringify(state, null, 2);
  }

  async function apply(action) {
    const result = await invoke("apply_input", { state, action });
    state = result.state;
    notice = "Applied " + action.kind + " deterministically.";
    render();
  }

  async function firstEmptyBuild(structure) {
    const cell = state?.cells?.find((candidate) => !candidate.structure);
    if (!cell) {
      notice = "No empty cell remains.";
      return render();
    }
    await apply({ kind:"build", x:cell.x, y:cell.y, structure });
  }

  async function onClick(event) {
    const action = event.target?.dataset?.action;
    if (!action || !state) return;
    try {
      if (action === "build-farm") await firstEmptyBuild("farm");
      else if (action === "build-generator") await firstEmptyBuild("generator");
      else if (action === "advance") await apply({ kind:"advance" });
      else if (action === "ai") {
        const result = await invoke("request_ai_move", { state });
        notice = result.available ? "AI suggestion: " + JSON.stringify(result.suggestion) : "AI unavailable: choose a provider in Powerbox.";
        render();
      } else if (action === "save") {
        const result = await invoke("export_save", { state });
        notice = "Portable save: " + result.digest;
        render();
      }
    } catch {
      notice = "The Host rejected the operation; inspect the structured Run or Binding status.";
      render();
    }
  }

  root.addEventListener("click", onClick);
  invoke("create_state", { title:"Tidelight Colony" })
    .then((result) => {
      if (disposed) return;
      state = result.state;
      notice = "Ready. Optional AI remains unbound until explicitly selected.";
      render();
    })
    .catch(() => {
      notice = "Simulation capability is unavailable for this Run.";
      render();
    });

  return () => {
    disposed = true;
    root.removeEventListener("click", onClick);
    root.replaceChildren();
  };
}
