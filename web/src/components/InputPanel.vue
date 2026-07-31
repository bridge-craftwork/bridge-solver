<script setup>
/**
 * The input: one small field for all three accepted forms, plus a file drop.
 *
 * Deliberately a single line rather than a textarea. What you paste is a LIN
 * record or a URL — a hundred characters of pipe-delimited machine text nobody
 * reads back — so giving it six rows of prominence spent the top of the page on
 * something with nothing to say. It still takes a multi-board file; a file that
 * size arrives by drop or picker rather than by being read on screen.
 */
import { computed, ref, watch } from 'vue'
import { EXAMPLE } from '../lib/example.js'

const props = defineProps({
  busy: { type: Boolean, default: false },
  error: { type: String, default: '' },
  /**
   * The hand the app is currently showing, so the field reflects it.
   *
   * Needed because a hand can arrive without being typed here — restored from the
   * last visit, or handed over by the URL. An empty box beside a rendered analysis
   * reads as though nothing were loaded, and Clear would have had nothing to clear.
   */
  hand: { type: String, default: '' },
})

const emit = defineEmits(['analyse'])

const text = ref(props.hand || '')
const dragging = ref(false)

// Follow the app when the hand changes from outside this component.
watch(
  () => props.hand,
  (next) => {
    if (next !== text.value) text.value = next || ''
  }
)
const fileInput = ref(null)

/** A pasted file is many lines; say so rather than showing them in one line. */
const multiline = computed(() => text.value.includes('\n'))
const lineCount = computed(() => text.value.split('\n').filter((l) => l.trim()).length)

function submit() {
  if (!text.value.trim() || props.busy) return
  emit('analyse', text.value)
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
  text.value = contents.join('\n')
  submit()
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
  emit('analyse', '')
}

function loadExample() {
  text.value = EXAMPLE
  submit()
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
    <h2 id="input-heading" class="sr-only">Paste a hand</h2>

    <div class="row">
      <label for="input-text" class="sr-only">
        A PBN board, a LIN record, or a BBO handviewer URL
      </label>
      <input
        v-if="!multiline"
        id="input-text"
        v-model="text"
        type="text"
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        placeholder="Paste here"
        title="A PBN board, a LIN record, or a BBO handviewer URL — or drop a file anywhere on this panel"
        @keydown.enter="submit"
      />
      <!-- A whole file pasted or dropped: the content is not worth reading, so
           report what it is instead of scrolling it past. -->
      <p v-else class="loaded">
        {{ lineCount }} lines loaded
        <button type="button" class="btn btn-quiet" @click="text = ''">edit</button>
      </p>

      <button type="button" class="btn btn-primary" :disabled="!text.trim() || busy" @click="submit">
        {{ busy ? 'Analysing…' : 'Analyse' }}
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

    <p v-if="error" class="error" role="alert">{{ error }}</p>
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

/*
 * Deliberately small. Nobody types a LIN record and nobody reads one back, so the
 * field only has to hold its own prompt — a full-width box was giving the top of
 * the page over to a hundred characters of machine text.
 */
input[type='text'] {
  flex: 0 0 auto;
  width: 11ch;
  min-width: 0;
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

.loaded {
  flex: 0 0 auto;
  margin: 0;
  font-size: 13px;
  color: var(--text-secondary);
}

/* One shape for all four controls: they are peers, and two of them reading as
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

.btn-quiet {
  padding: 2px 8px;
  font-size: 12px;
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
