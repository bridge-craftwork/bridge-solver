<script setup>
/** The paste box: one input for all three accepted forms, plus a file drop. */
import { ref } from 'vue'

const props = defineProps({
  busy: { type: Boolean, default: false },
  error: { type: String, default: '' },
})

const emit = defineEmits(['analyse'])

const text = ref('')
const dragging = ref(false)

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

/** Empty the box and tell the app to drop the current board with it. */
function clear() {
  text.value = ''
  emit('analyse', '')
}

function loadExample() {
  // A real BBO board, claimed after 28 cards, with a redoubled call in the
  // auction — enough shape to show every part of the page at once.
  text.value =
    'qx|o6|pn|aam135,usvi,kemistry,jelsma|st||md|4SQH5AD28JKAC257JA,S379KH278QKD69C9T,S26AH369TJD5C38QK,|rh||ah|Board 6|sv|e|mb|p|mb|1D|mb|d|mb|r|mb|1S|mb|3C|mb|p|mb|4C|mb|p|mb|4H|an|0 or 3 kc|mb|p|mb|6C|mb|p|mb|p|mb|p|pc|HK|pc|H3|pc|H4|pc|HA|pc|CA|pc|CT|pc|C3|pc|C4|pc|C2|pc|C9|pc|CK|pc|C6|pc|D5|pc|D3|pc|DA|pc|D6|pc|H5|pc|HQ|pc|H6|pc|S4|pc|D9|pc|S2|pc|DQ|pc|DK|pc|DJ|pc|S3|pc|S6|pc|D4|mc|12|pg||'
  submit()
}
</script>

<template>
  <section class="panel" aria-labelledby="input-heading">
    <h2 id="input-heading">Paste a hand</h2>
    <p class="hint">
      A PBN board or file, a LIN record or file, or a BBO handviewer URL. A LIN or
      PBN file with several boards gives you all of them.
    </p>

    <div
      class="drop"
      :class="{ dragging }"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop.prevent="onDrop"
    >
      <label for="input-text" class="sr-only">Hand to analyse</label>
      <textarea
        id="input-text"
        v-model="text"
        rows="6"
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        placeholder="Paste here — or drop a .lin or .pbn file anywhere in this box"
        @keydown.ctrl.enter="submit"
        @keydown.meta.enter="submit"
      />
    </div>

    <div class="actions">
      <button type="button" class="btn primary" :disabled="!text.trim() || busy" @click="submit">
        {{ busy ? 'Analysing…' : 'Analyse' }}
      </button>

      <label class="btn secondary file-btn">
        Choose a file
        <input type="file" accept=".lin,.pbn,.txt,text/plain" multiple @change="onPick" />
      </label>

      <button type="button" class="btn link" @click="loadExample">Try an example</button>

      <button v-if="text" type="button" class="btn link" @click="clear">Clear</button>
    </div>

    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </section>
</template>

<style scoped>
.panel {
  background: var(--bg-white);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  padding: 16px 18px;
}

h2 {
  font-size: 18px;
}

.hint {
  margin: 0 0 10px;
  font-size: 13px;
  color: var(--text-secondary);
}

.drop {
  border: 2px dashed transparent;
  border-radius: var(--radius-button);
  transition: border-color 0.15s, background 0.15s;
}

.drop.dragging {
  border-color: var(--green);
  background: #f0faf5;
}

textarea {
  width: 100%;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.5;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-button);
  resize: vertical;
  background: #fcfcfb;
  color: var(--text);
}

textarea:focus {
  outline: 2px solid var(--green);
  outline-offset: -1px;
}

.actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  margin-top: 10px;
}

.btn {
  font: inherit;
  font-size: 14px;
  border-radius: var(--radius-button);
  padding: 7px 14px;
  cursor: pointer;
  border: 1px solid transparent;
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.btn.primary {
  background: var(--green);
  color: #fff;
  font-weight: 600;
}
.btn.primary:not(:disabled):hover {
  background: var(--green-hover);
}

.btn.secondary {
  background: var(--bg-white);
  border-color: var(--border);
  color: var(--text);
}
.btn.secondary:hover {
  border-color: var(--green);
}

.btn.link {
  background: transparent;
  color: var(--green-hover);
  padding: 7px 6px;
  text-decoration: underline;
}

.file-btn {
  position: relative;
  overflow: hidden;
}

.file-btn input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
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
