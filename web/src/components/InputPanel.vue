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
import { computed, ref } from 'vue'

const props = defineProps({
  busy: { type: Boolean, default: false },
  error: { type: String, default: '' },
})

const emit = defineEmits(['analyse'])

/**
 * A real BBO board with mistakes in it, so the example shows the page working
 * rather than a clean hand with nothing to look at. Board 3 of a club game: 3NT
 * claimed after 41 cards, five costed errors on both sides.
 *
 * The opponents' names are replaced with placeholders — this page is public, and
 * the example is a permanent fixture of it rather than something a user chose to
 * paste.
 */
const EXAMPLE =
  'https://www.bridgebase.com/tools/handviewer.html?lin=pn%7Csoffiadan%2COpponent+1%2COrion04%2COpponent+2%7Cst%7C%7Cmd%7C1SKT43H652DA984C94%2CSA5HAK97D732CKQ72%2CSJ98HQT83DK6CJ853%2CSQ762HJ4DQJT5CAT6%7Csv%7Ce%7Crh%7C%7Cah%7CBoard+3%7Cmb%7CP%7Cmb%7C1N%7Cmb%7CP%7Cmb%7C2C%7Cmb%7CP%7Cmb%7C2H%7Cmb%7CP%7Cmb%7C3N%7Cmb%7CP%7Cmb%7CP%7Cmb%7CP%7Cpc%7CC3%7Cpc%7CCT%7Cpc%7CC4%7Cpc%7CC2%7Cpc%7CDQ%7Cpc%7CD4%7Cpc%7CD2%7Cpc%7CDK%7Cpc%7CSJ%7Cpc%7CS2%7Cpc%7CS3%7Cpc%7CSA%7Cpc%7CD3%7Cpc%7CD6%7Cpc%7CDJ%7Cpc%7CDA%7Cpc%7CC9%7Cpc%7CC7%7Cpc%7CC5%7Cpc%7CCA%7Cpc%7CDT%7Cpc%7CD8%7Cpc%7CD7%7Cpc%7CS8%7Cpc%7CHJ%7Cpc%7CH6%7Cpc%7CH7%7Cpc%7CHQ%7Cpc%7CS9%7Cpc%7CSQ%7Cpc%7CSK%7Cpc%7CS5%7Cpc%7CST%7Cpc%7CH9%7Cpc%7CC8%7Cpc%7CS6%7Cpc%7CD9%7Cpc%7CCQ%7Cpc%7CCJ%7Cpc%7CD5%7Cpc%7CH5%7Cmc%7C7%7C'

const text = ref('')
const dragging = ref(false)
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
        placeholder="Paste a PBN board, a LIN record, or a BBO handviewer URL — or drop a file here"
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

input[type='text'] {
  flex: 1 1 320px;
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
  flex: 1 1 320px;
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
