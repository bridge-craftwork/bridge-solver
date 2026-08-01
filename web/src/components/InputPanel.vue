<script setup>
/**
 * The input: no field at all, just the ways a hand actually arrives.
 *
 * There was a text field here. It went, because nobody reads back what it
 * held: a LIN record or a handviewer URL is a hundred-odd characters of
 * pipe-delimited machine text, and showing it spent the top of the page
 * displaying something with nothing to say. What matters is *that* a hand
 * loaded and which one — so this shows that instead.
 *
 * Three ways in, and the ordering below is deliberate:
 *
 * 1. **Pasting anywhere on the page.** The primary path, and the one that works
 *    everywhere. A `paste` event carries its own `clipboardData`, and per the
 *    Clipboard API spec an event handler may read it when "the action that
 *    triggers the event is invoked from the user-agent's own user interface" —
 *    no permission check, unlike `navigator.clipboard.readText()`, which must
 *    "check clipboard read permission" and rejects without it. So Ctrl/⌘-V
 *    works in an embed, in Firefox, and with no prompt, in all the cases the
 *    Paste button below does not.
 * 2. **The Paste button**, for anyone reaching for a button rather than a
 *    keystroke — and on touch, where there is no ⌘V. This one *does* go through
 *    the async API, so it needs the `clipboard-read` Permissions Policy and is
 *    hidden where the API is absent. Expect it to fail inside an embed.
 * 3. **A file**, dropped anywhere on the panel or chosen.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { EXAMPLE } from '../lib/example.js'
import { INPUT_KINDS, detectKind } from '../lib/input.js'

const props = defineProps({
  busy: { type: Boolean, default: false },
  error: { type: String, default: '' },
  /**
   * The hand the app is currently showing.
   *
   * Still needed with the field gone: a hand can arrive without passing through
   * this component — restored from the last visit, or handed over by the URL —
   * and the summary line has to describe that one too.
   */
  hand: { type: String, default: '' },
})

const emit = defineEmits(['analyse'])

const text = ref(props.hand || '')
const dragging = ref(false)
const fileInput = ref(null)

watch(
  () => props.hand,
  (next) => {
    if (next !== text.value) text.value = next || ''
  }
)

/**
 * Whether this browser will let a *button* read the clipboard.
 *
 * `navigator.clipboard` is undefined outside a secure context, so this covers
 * plain HTTP as well as browsers without the API. Pasting by keystroke does not
 * depend on any of this and is always available.
 */
const canPaste = typeof navigator !== 'undefined' && !!navigator.clipboard?.readText

/** ⌘ on a Mac, Ctrl everywhere else — the hint is useless if it names the wrong key. */
const pasteKey =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.userAgent) ? '⌘V' : 'Ctrl+V'

/** A failure from the Paste button, shown where a parse error would be. */
const pasteError = ref('')
const shownError = computed(() => pasteError.value || props.error)

const lineCount = computed(() => text.value.split('\n').filter((l) => l.trim()).length)

/**
 * What is loaded, in words — the field's replacement.
 *
 * Says the *kind* of thing rather than echoing it, which is the whole point of
 * removing the box. Reuses `detectKind` so this can never disagree with what
 * the parser then does with it.
 */
const summary = computed(() => {
  if (!text.value.trim()) return ''
  if (lineCount.value > 1) return `${lineCount.value} lines loaded`
  switch (detectKind(text.value)) {
    case INPUT_KINDS.URL:
      return 'Handviewer link loaded'
    case INPUT_KINDS.LIN:
      return 'LIN record loaded'
    case INPUT_KINDS.PBN:
      return 'PBN board loaded'
    case INPUT_KINDS.DEAL:
      return 'Deal loaded'
    default:
      return 'Loaded'
  }
})

function submit() {
  if (!text.value.trim() || props.busy) return
  pasteError.value = ''
  emit('analyse', text.value)
}

/** Take some text as the new hand and analyse it straight away. */
function accept(value) {
  const trimmed = String(value || '').trim()
  if (!trimmed) return false
  text.value = trimmed
  submit()
  return true
}

/**
 * Pasting anywhere on the page loads a hand.
 *
 * Bound to the window rather than to a control, because with no field there is
 * nothing to aim at — the reader arrives holding a hand and presses paste, and
 * that should simply work.
 *
 * Skipped when the paste is aimed at something editable, so a future input on
 * the page is not hijacked by this one.
 */
function onPaste(event) {
  if (props.busy) return
  const target = event.target
  if (target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName || '')) return

  const pasted = (event.clipboardData || window.clipboardData)?.getData('text')
  if (accept(pasted)) event.preventDefault()
}

onMounted(() => window.addEventListener('paste', onPaste))
onUnmounted(() => window.removeEventListener('paste', onPaste))

/**
 * The button path, for touch and for anyone who would rather click.
 *
 * Every failure lands on the same advice because from here they are
 * indistinguishable and the remedy is identical: a refused permission, an
 * iframe without `clipboard-read`, and a dismissed prompt all throw
 * `NotAllowedError`. Pressing the paste keystroke works in all three.
 */
async function pasteAndAnalyse() {
  if (props.busy) return
  try {
    const clip = (await navigator.clipboard.readText()).trim()
    if (!clip) {
      pasteError.value = 'There is nothing on the clipboard to paste.'
      return
    }
    accept(clip)
  } catch {
    pasteError.value = `This browser would not let the page read the clipboard — that is normal when this page is embedded in another site. Press ${pasteKey} instead, which always works.`
  }
}

/**
 * Read dropped or chosen files.
 *
 * `FileList` is a live collection the browser empties out from under an async
 * handler, so it is snapshotted with `Array.from` before the first `await`.
 * Without that you silently keep only the first file.
 */
async function readFiles(fileList) {
  const files = Array.from(fileList || [])
  if (!files.length) return
  const contents = await Promise.all(files.map((f) => f.text()))
  accept(contents.join('\n'))
}

function onDrop(event) {
  dragging.value = false
  readFiles(event.dataTransfer?.files)
}

function onPick(event) {
  readFiles(event.target.files)
  // Let the same file be chosen twice in a row.
  event.target.value = ''
}

function clear() {
  text.value = ''
  pasteError.value = ''
  emit('analyse', '')
}

function loadExample() {
  accept(EXAMPLE)
}

/**
 * What is currently in the field.
 *
 * Almost always empty: a paste is taken straight out of the event and the box
 * blanks, so this only holds something while it is being typed by hand — a bare
 * deal string, realistically, since nobody types a LIN record.
 */
const typed = ref('')

/**
 * Take a paste out of the field without ever showing it.
 *
 * The field exists because a touchscreen has no paste keystroke: holding a real
 * input is the only way to raise the system Paste, and on an iPad the button
 * above cannot stand in for it — Chrome there would not hand the page the
 * clipboard at all. But the reason the field went away in the first place still
 * holds, so the text is read from the event and the default insertion is
 * prevented. Nothing is ever rendered into the box.
 */
function onPasteInto(event) {
  const pasted = (event.clipboardData || window.clipboardData)?.getData('text')
  if (!pasted?.trim()) return
  event.preventDefault()
  typed.value = ''
  accept(pasted)
}

/** Enter on something typed by hand — the one case the paste path misses. */
function submitTyped() {
  if (!typed.value.trim()) return
  const value = typed.value
  typed.value = ''
  accept(value)
}
</script>

<template>
  <section
    class="panel"
    :class="{ dragging }"
    aria-labelledby="input-heading"
    @dragover.prevent="dragging = true"
    @dragleave.prevent="dragging = false"
    @drop.prevent="onDrop"
  >
    <h2 id="input-heading" class="sr-only">Load a hand</h2>

    <div class="row">
      <!--
        A real field, because on a touch device it is the only thing that can be
        held to raise the system Paste. It is a paste *target* rather than a
        display, though: what lands in it is taken and the box blanks again, so
        nobody has to look at a LIN record.
      -->
      <label for="input-text" class="sr-only">
        A PBN board, a LIN record, or a BBO handviewer URL
      </label>
      <input
        id="input-text"
        v-model="typed"
        type="text"
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        autocorrect="off"
        :disabled="busy"
        placeholder="Paste here"
        title="Paste a hand here — hold to bring up Paste on a touchscreen"
        @paste="onPasteInto"
        @keydown.enter="submitTyped"
      />

      <p class="status" :class="{ empty: !summary }" aria-live="polite">
        <template v-if="summary">{{ summary }}</template>
        <template v-else>Paste a hand here, or drop a file</template>
      </p>

      <button
        v-if="canPaste"
        type="button"
        class="btn"
        :class="{ 'btn-primary': !text.trim() }"
        :disabled="busy"
        @click="pasteAndAnalyse"
      >
        Paste
      </button>
      <button
        v-if="text.trim()"
        type="button"
        class="btn btn-primary"
        :disabled="busy"
        @click="submit"
      >
        {{ busy ? 'Analysing…' : 'Analyse again' }}
      </button>
      <button type="button" class="btn" @click="fileInput.click()">Choose a file</button>
      <button type="button" class="btn" @click="loadExample">Try an example</button>
      <button type="button" class="btn" :disabled="!text" @click="clear">Clear</button>

      <input
        ref="fileInput"
        class="sr-only"
        type="file"
        accept=".lin,.pbn,.txt,text/plain"
        multiple
        tabindex="-1"
        @change="onPick"
      />
    </div>

    <p v-if="shownError" class="error" role="alert">{{ shownError }}</p>
  </section>
</template>

<style scoped>
.panel {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: 12px 14px;
  border-style: solid;
  transition: border-color 0.15s, background 0.15s;
}

.panel.dragging {
  border-color: var(--green);
  border-style: dashed;
  background: #f0faf5;
}

.row {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

/* Where the field used to be, at the same weight: present enough to read as
   the panel's subject, quiet enough not to compete with the buttons. */
/*
 * Small, because it is a target rather than a display — it never holds more
 * than a moment's worth of text. But not *too* small: it has to be comfortable
 * to press and hold on a touchscreen to raise the system Paste, which is the
 * whole reason it exists, so the height clears the usual 34px tap minimum even
 * though the width does not need to.
 */
input[type='text'] {
  flex: 0 0 auto;
  width: 10ch;
  min-width: 0;
  min-height: 34px;
  font-family: var(--font-mono);
  font-size: 12px;
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-button);
  background: #fcfcfb;
  color: var(--text);
}

input[type='text']:focus {
  outline: 2px solid var(--green);
  outline-offset: -1px;
}

.status {
  flex: 0 1 auto;
  margin: 0 4px 0 0;
  font-size: 13px;
  color: var(--text);
  font-weight: 600;
}

.status.empty {
  color: var(--text-secondary);
  font-weight: 400;
}

/* One shape for all the controls: they are peers, and two of them reading as
   links made them look like a different kind of thing. */
.btn {
  font: inherit;
  font-size: 13px;
  border-radius: var(--radius-button);
  padding: 7px 13px;
  cursor: pointer;
  border: 1px solid var(--border);
  background: var(--bg-white);
  color: var(--text);
  white-space: nowrap;
}

.btn:not(:disabled):hover {
  border-color: var(--green);
}

.btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.btn-primary {
  background: var(--green);
  border-color: var(--green);
  color: #fff;
  font-weight: 600;
}

.btn-primary:not(:disabled):hover {
  background: var(--green-hover);
  border-color: var(--green-hover);
}

.error {
  margin: 10px 0 0;
  padding: 8px 10px;
  background: #fdecea;
  border: 1px solid #f5c6c2;
  border-radius: var(--radius-button);
  color: #a1281f;
  font-size: 13px;
  white-space: pre-wrap;
}
</style>
