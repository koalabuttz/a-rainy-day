const canvas = document.querySelector("#room");
const status = document.querySelector("#status");
const dialogue = document.querySelector("#dialogue");
const keys = new Map([
  ["KeyW", 1], ["ArrowUp", 1], ["KeyS", 2], ["ArrowDown", 2],
  ["KeyA", 4], ["ArrowLeft", 4], ["KeyD", 8], ["ArrowRight", 8],
]);
const directions = { up: 1, down: 2, left: 4, right: 8 };
const heldKeys = new Set();
const heldButtons = new Map();

function inputMask() {
  let mask = 0;
  for (const key of heldKeys) mask |= keys.get(key);
  for (const button of heldButtons.values()) mask |= directions[button.dataset.direction];
  return mask;
}

function clearInput() {
  heldKeys.clear();
  for (const button of heldButtons.values()) button.removeAttribute("data-held");
  heldButtons.clear();
}

function bindControls() {
  window.addEventListener("keydown", (event) => {
    if (!keys.has(event.code) || !dialogue.hidden || event.target instanceof HTMLInputElement ||
        event.ctrlKey || event.metaKey || event.altKey) return;
    event.preventDefault();
    heldKeys.add(event.code);
  });
  window.addEventListener("keyup", (event) => heldKeys.delete(event.code));
  window.addEventListener("blur", clearInput);
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) clearInput();
  });
  for (const button of document.querySelectorAll("[data-direction]")) {
    button.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      button.setPointerCapture(event.pointerId);
      heldButtons.set(event.pointerId, button);
      button.setAttribute("data-held", "");
    });
    function release(event) {
      heldButtons.delete(event.pointerId);
      button.toggleAttribute("data-held", [...heldButtons.values()].includes(button));
    }
    button.addEventListener("pointerup", release);
    button.addEventListener("pointercancel", release);
    button.addEventListener("lostpointercapture", release);
    button.addEventListener("contextmenu", (event) => event.preventDefault());
  }
}

function bindDialogue(room, draw) {
  const interact = document.querySelector("#interact");
  const controls = document.querySelector(".directions");
  const speaker = document.querySelector("#speaker");
  const line = document.querySelector("#line");
  const choices = document.querySelector("#choices");
  const form = document.querySelector("#say");
  const question = document.querySelector("#question");
  const reply = document.querySelector("#reply");
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let rendered = "";
  let lastTarget = 0;

  function read(slot) {
    const length = room.room_text_len(slot);
    return decoder.decode(new Uint8Array(room.memory.buffer, room.room_text(slot), length));
  }

  function refresh() {
    const target = room.room_interaction();
    if (target !== lastTarget) {
      interact.hidden = target === 0;
      interact.textContent = target === 2 ? "Listen" : "Talk";
      lastTarget = target;
    }

    const name = read(0);
    const text = read(1);
    const labels = [2, 3, 4, 5].map(read).filter(Boolean);
    const signature = JSON.stringify([name, text, labels]);
    if (signature === rendered) return;
    rendered = signature;
    reply.hidden = true;
    dialogue.hidden = !name;
    controls.hidden = Boolean(name);
    form.hidden = labels.length < 2;
    speaker.textContent = name;
    line.textContent = text;
    choices.replaceChildren(...labels.map((label, index) => {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      button.addEventListener("click", () => {
        room.room_select(index);
        draw();
        refresh();
        (dialogue.hidden ? interact : choices.firstElementChild)?.focus();
      });
      return button;
    }));
  }

  function open() {
    if (interact.hidden) return;
    clearInput();
    room.room_open_interaction();
    draw();
    refresh();
    choices.firstElementChild?.focus();
  }

  interact.addEventListener("click", open);
  window.addEventListener("keydown", (event) => {
    if (event.code === "KeyE" && !event.repeat && event.target !== question &&
        !event.ctrlKey && !event.metaKey && !event.altKey) open();
  });
  question.addEventListener("focus", clearInput);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const bytes = encoder.encode(question.value);
    if (bytes.length <= room.room_input_capacity()) {
      const input = new Uint8Array(room.memory.buffer, room.room_input_buffer(), bytes.length);
      input.set(bytes);
      if (room.room_submit_text(bytes.length)) {
        question.value = "";
        reply.hidden = true;
        draw();
        refresh();
        if (dialogue.hidden && !interact.hidden) interact.focus();
        return;
      }
    }
    reply.textContent = "Try asking about a room, the food, or the staff.";
    reply.hidden = false;
  });
  return refresh;
}

async function openRoom() {
  const encoded = document.querySelector("#room-wasm").textContent.trim();
  const bytes = Uint8Array.from(atob(encoded), (char) => char.charCodeAt(0));
  const { instance } = await WebAssembly.instantiate(bytes);
  const room = instance.exports;
  const width = room.room_width();
  const height = room.room_height();
  const pixels = room.room_init();
  if (!pixels) throw new Error("Could not load the room art");
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) throw new Error("Canvas 2D is unavailable");
  canvas.width = width;
  canvas.height = height;
  context.imageSmoothingEnabled = false;

  const draw = () => {
    const rgba = new Uint8ClampedArray(room.memory.buffer, pixels, width * height * 4);
    context.putImageData(new ImageData(rgba, width, height), 0, 0);
  };
  bindControls();
  const refreshDialogue = bindDialogue(room, draw);
  draw();
  refreshDialogue();
  status.hidden = true;

  const tickMs = 1000 / room.room_ticks_per_second();
  let lastTime = performance.now();
  let elapsed = 0;
  function frame(now) {
    // Limit catch-up after backgrounding so old time does not move the player.
    elapsed += Math.min(Math.max(now - lastTime, 0), tickMs * 4);
    lastTime = now;
    if (elapsed >= tickMs) {
      const mask = inputMask();
      while (elapsed >= tickMs) {
        room.room_tick(mask);
        elapsed -= tickMs;
      }
      draw();
      refreshDialogue();
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

openRoom().catch((error) => {
  status.textContent = "The inn could not open. Try reopening the page.";
  console.error(error);
});
