import "@wisp/ui/styles.css";
import { createWebKitTransport, IpcClient, isHosted } from "@wisp/contracts";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { NotHosted } from "./app/NotHosted";
import { setIpcClient } from "./services/ipc";

const container = document.getElementById("root");
if (!container) throw new Error("#root element missing from index.html");
const root = createRoot(container);

if (isHosted()) {
  setIpcClient(new IpcClient(createWebKitTransport()));
  root.render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
} else {
  root.render(<NotHosted />);
}
