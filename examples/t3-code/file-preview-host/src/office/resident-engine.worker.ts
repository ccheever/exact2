// The Word engine's own worker (@betteroffice/docx's yrs residentEngineWorker.mjs, which the engine starts beside its
// code as `./residentEngineWorker.mjs`), with the page's packed files readable in it.
import "../shared/packed";
import "@betteroffice/docx/dist/yrs/residentEngineWorker.mjs";
