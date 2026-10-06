import { formatMb, formatPct } from "./format";

test("formatMb usa MB debajo de 1 GB y GB con un decimal arriba", () => {
  expect(formatMb(729)).toBe("729 MB");
  expect(formatMb(1024)).toBe("1.0 GB");
  expect(formatMb(16368)).toBe("16.0 GB");
  expect(formatMb(null)).toBe("sin datos");
});

test("formatPct redondea y tolera totales en cero", () => {
  expect(formatPct(7819, 16368)).toBe("48 %");
  expect(formatPct(5, 0)).toBe("0 %");
});
