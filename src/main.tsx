import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// Галерея примитивов: только в dev и только по ?gallery=1.
// В продакшене import.meta.env.DEV становится false, и динамический import в бандл не попадает.
if (import.meta.env.DEV && new URLSearchParams(window.location.search).get("gallery") === "1") {
  import("./ui/Gallery").then(({ default: Gallery }) => {
    root.render(
      <React.StrictMode>
        <Gallery />
      </React.StrictMode>,
    );
  });
} else {
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
