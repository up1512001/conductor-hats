/** One accessible transient status strip shared by project and chat screens. */

export function noticeFor(node: HTMLElement): (text: string, error?: boolean) => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return (text: string, error = false): void => {
    if (timer) clearTimeout(timer);
    node.textContent = text;
    node.hidden = !text;
    node.classList.toggle("error", error);
    if (!text) return;
    timer = setTimeout(() => {
      node.hidden = true;
      timer = null;
    }, 6000);
  };
}
