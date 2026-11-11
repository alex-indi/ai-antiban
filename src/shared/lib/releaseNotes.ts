export interface ParsedChangelogSection {
  title?: string;
  items: string[];
}

export function parseReleaseNotes(notes: string): ParsedChangelogSection[] {
  if (!notes || !notes.trim()) {
    return [{ items: ["Улучшения стабильности и безопасности сетевого шлюза."] }];
  }

  const lines = notes.split("\n");
  const sections: ParsedChangelogSection[] = [];
  let currentTitle: string | undefined = undefined;
  let currentItems: string[] = [];

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;

    if (
      line.includes(".dmg") ||
      line.includes(".exe") ||
      line.includes(".tar.gz") ||
      line.includes(".zip") ||
      line.startsWith("---") ||
      line.startsWith("***") ||
      /^#{1,6}\s*(Скачать|Загрузка|Ссылки|Assets|Downloads)/i.test(line)
    ) {
      continue;
    }

    if (line.startsWith("#")) {
      if (currentItems.length > 0) {
        sections.push({ title: currentTitle, items: currentItems });
        currentItems = [];
      }
      currentTitle = line.replace(/^#+\s*/, "").replace(/[*_]/g, "").trim();
      continue;
    }

    if (/^[-*•]\s+/.test(line)) {
      const cleanItem = line.replace(/^[-*•]\s+/, "").replace(/[*_]/g, "").trim();
      if (cleanItem) {
        currentItems.push(cleanItem);
      }
    } else if (/^\d+\.\s+/.test(line)) {
      const cleanItem = line.replace(/^\d+\.\s+/, "").replace(/[*_]/g, "").trim();
      if (cleanItem) {
        currentItems.push(cleanItem);
      }
    } else if (currentItems.length > 0 && !line.startsWith(">") && !line.includes("http")) {
      const cleanItem = line.replace(/[*_]/g, "").trim();
      if (cleanItem.length > 3 && !cleanItem.toLowerCase().startsWith("для новых")) {
        currentItems.push(cleanItem);
      }
    }
  }

  if (currentItems.length > 0) {
    sections.push({ title: currentTitle, items: currentItems });
  }

  if (sections.length === 0 || sections.every((s) => s.items.length === 0)) {
    return [{ items: [notes.slice(0, 300).trim()] }];
  }

  return sections;
}
