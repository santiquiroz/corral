import { render, screen } from "@testing-library/react";
import App from "./App";

test("muestra el nombre del producto", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Corral" })).toBeInTheDocument();
});
