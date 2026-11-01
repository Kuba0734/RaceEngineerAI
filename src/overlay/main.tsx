import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "../styles/index.css";
import "../i18n";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Overlay from "./Overlay";

const root = document.getElementById("overlay-root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <Overlay />
    </StrictMode>,
  );
}
