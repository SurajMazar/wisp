import { useMemo } from "react";
import { RouterProvider } from "react-router";
import { Bootstrap } from "./providers/Bootstrap";
import { createRouter } from "./router/routes";

export function App() {
  const router = useMemo(() => createRouter(), []);
  return (
    <Bootstrap>
      <RouterProvider router={router} />
    </Bootstrap>
  );
}
