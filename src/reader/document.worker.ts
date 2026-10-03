import { compileReaderDocument } from "./document";

self.onmessage = (event: MessageEvent<string>) => {
  try {
    self.postMessage({ document: compileReaderDocument(event.data) });
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : String(error) });
  }
};
