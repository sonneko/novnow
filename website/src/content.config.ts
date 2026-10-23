import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

// docs/ から scripts/docs-site.py が写したものだけを読む（手で書いた原稿は置かない）。
// タイトル・並び順・元のパスは生成器が frontmatter に入れる。
const docs = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string(),
    group: z.enum(["users", "specifications"]),
    order: z.number(),
    source: z.string(),
    lang: z.literal("ja").default("ja"),
  }),
});

export const collections = { docs };
