import type { ParsedSearch, Category } from "./types";

const standalone = (expression: string, flags = "gi") =>
  new RegExp(`(^|[^\\w.\\-])(${expression})(?=$|[^\\w.\\-])`, flags);

const aliases: [RegExp, Category, string][] = [
  [standalone("screenshots?"), "images", "Screenshots"],
  [standalone("images?|photos?|pictures?"), "images", "Images"],
  [standalone("videos?|movies?"), "videos", "Videos"],
  [standalone("music|audio|songs?"), "music", "Music"],
  [standalone("documents?|docs?"), "documents", "Documents"],
  [standalone("archives?|zips?"), "archives", "Archives"],
  [standalone("apps?|applications?|installers?"), "apps", "Apps"],
];

export function parseSearch(input: string, now = new Date()): ParsedSearch {
  let text = input.trim();
  const quoted: string[] = [];
  text = text.replace(/"([^"]+)"/g, (_, value: string) => {
    quoted.push(value);
    return `__quoted_${quoted.length - 1}__`;
  });
  const query: ParsedSearch["query"] = {};
  const chips: string[] = [];
  for (const [pattern, category, label] of aliases) {
    pattern.lastIndex = 0;
    if (pattern.test(text)) {
      query.category = category;
      if (label === "Screenshots") query.screenshot = true;
      chips.push(label);
      text = text.replace(pattern, " ");
      break;
    }
  }
  text = text.replace(
    standalone("pdfs?|pngs?|jpe?gs?|mp4s?|csvs?|docx?|xlsx?|ppts?|txts?"),
    (_match, _prefix: string, word: string) => {
      const lower = word.toLowerCase();
      const extension = lower === "xls" ? "xls" : lower.replace(/s$/, "");
      query.terms = [`ext:${extension}`];
      chips.push(extension.toUpperCase());
      return " ";
    },
  );
  const size =
    /\b(larger than|bigger than|over|above|at least|smaller than|under|below|less than)\s*(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb|bytes?|b)\b/i;
  const sizeMatch = text.match(size);
  if (sizeMatch) {
    const unit = sizeMatch[3].toLowerCase();
    const factor =
      (
        { kb: 1024, mb: 1024 ** 2, gb: 1024 ** 3, tb: 1024 ** 4 } as Record<
          string,
          number
        >
      )[unit] ?? 1;
    const bytes = Math.round(Number(sizeMatch[2]) * factor);
    const min = /larger|bigger|over|above|at least/.test(
      sizeMatch[1].toLowerCase(),
    );
    query[min ? "min_size" : "max_size"] = bytes;
    chips.push(`${min ? "≥" : "≤"} ${sizeMatch[2]} ${unit.toUpperCase()}`);
    text = text.replace(size, " ");
  }
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const dates: [RegExp, Date, Date, string][] = [
    [
      standalone("last month", "i"),
      new Date(today.getFullYear(), today.getMonth() - 1, 1),
      new Date(today.getFullYear(), today.getMonth(), 1),
      "Last month",
    ],
    [
      standalone("this month", "i"),
      new Date(today.getFullYear(), today.getMonth(), 1),
      new Date(today.getFullYear(), today.getMonth() + 1, 1),
      "This month",
    ],
    [
      standalone("last week", "i"),
      new Date(
        today.getFullYear(),
        today.getMonth(),
        today.getDate() - ((today.getDay() + 6) % 7) - 7,
      ),
      new Date(
        today.getFullYear(),
        today.getMonth(),
        today.getDate() - ((today.getDay() + 6) % 7),
      ),
      "Last week",
    ],
    [
      standalone("this week", "i"),
      new Date(
        today.getFullYear(),
        today.getMonth(),
        today.getDate() - ((today.getDay() + 6) % 7),
      ),
      new Date(
        today.getFullYear(),
        today.getMonth(),
        today.getDate() - ((today.getDay() + 6) % 7) + 7,
      ),
      "This week",
    ],
    [
      standalone("yesterday", "i"),
      new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1),
      today,
      "Yesterday",
    ],
    [
      standalone("today", "i"),
      today,
      new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1),
      "Today",
    ],
  ];
  for (const [pattern, from, to, label] of dates) {
    if (pattern.test(text)) {
      query.date_from = Math.floor(from.getTime() / 1000);
      query.date_to = Math.floor(to.getTime() / 1000);
      chips.push(label);
      text = text.replace(pattern, " ");
      break;
    }
  }
  // Remove connective words only when a structured clause was consumed.
  if (chips.length)
    text = text.replace(
      standalone("from|files?|made|modified|in|the|and"),
      " ",
    );
  const terms =
    text
      .match(/\S+/g)
      ?.map((t) =>
        t.replace(
          /__quoted_(\d+)__/g,
          (_, index: string) => quoted[Number(index)],
        ),
      ) ?? [];
  query.terms = [...(query.terms ?? []), ...terms];
  return { query, chips, terms };
}

export function formatBytes(bytes: number, decimals = 1): string {
  if (!bytes) return "0 B";
  const index = Math.min(
    Math.floor(Math.log(Math.max(bytes, 1)) / Math.log(1024)),
    4,
  );
  return `${(bytes / 1024 ** index).toLocaleString(undefined, { maximumFractionDigits: index ? decimals : 0 })} ${["B", "KB", "MB", "GB", "TB"][index]}`;
}
