import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { NotifyProvider } from "./lib/notify";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <NotifyProvider>
      <App />
    </NotifyProvider>
  </React.StrictMode>,
);
