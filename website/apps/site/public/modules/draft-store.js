const DATABASE = "tohseno-private-intentions";
const STORE = "drafts";
const KEY = "active";

export function openDraftStore() {
  if (!globalThis.indexedDB) return Promise.reject(new Error("IndexedDB is unavailable"));
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE)) request.result.createObjectStore(STORE);
    };
    request.onerror = () => reject(request.error || new Error("Could not open IndexedDB"));
    request.onsuccess = () => resolve({
      load: () => transaction(request.result, "readonly", (store) => store.get(KEY)),
      save: (draft) => transaction(request.result, "readwrite", (store) => store.put(draft, KEY)),
      clear: () => transaction(request.result, "readwrite", (store) => store.delete(KEY)),
    });
  });
}

function transaction(database, mode, operation) {
  return new Promise((resolve, reject) => {
    const tx = database.transaction(STORE, mode);
    const result = operation(tx.objectStore(STORE));
    tx.oncomplete = () => resolve(result.result);
    tx.onerror = () => reject(tx.error || result.error || new Error("IndexedDB transaction failed"));
    tx.onabort = () => reject(tx.error || new Error("IndexedDB transaction was aborted"));
  });
}
