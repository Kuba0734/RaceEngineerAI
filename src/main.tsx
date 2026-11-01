import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./styles/index.css";
import "./i18n";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./app/App";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
