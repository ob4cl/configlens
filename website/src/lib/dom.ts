export function element<T extends HTMLElement = HTMLElement>(
  selector: string,
): T {
  const found = document.querySelector<T>(selector);
  if (!found) throw new Error(`Missing required element: ${selector}`);
  return found;
}
export function text(selector: string, value: string) {
  element(selector).textContent = value;
}
