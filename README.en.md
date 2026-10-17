# novnow — Next-Generation World-Building Management Systems

`novnow` is a suite of tools designed to cultivate and expand your own fictional worlds—whether for full-length novels, light novels, web fiction, visual novels, or scenario scripts.

The novnow editor, built to manage the novnow format, is available as a desktop application (Windows / macOS / Linux) and in web browsers.

![](docs/screenshot-m7.png)

---

## What is the novnow format?

The "novnow format" is a file format designed to manage the world-building and story that live inside your mind.

Just as PDF is a format for managing documents, novnow is a format for managing fictional universes. Simply by sending a novnow file, you can communicate your entire world to others.

It is also ideal when multiple creators want to build a single shared world together.

Furthermore, it supports extensions designed to convey your world directly to AI systems.

## What does it look like in practice?

- Character modes and role-language (speech-style) settings
- Terminology glossary
- Idea stockpiles
- Manuscript text
- Overall plots, chapter plots, and episode plots
- Reference materials
- Character icons

All of the above information can be integrated and managed in one place.

For example, manuscript text is written like this:

```
@01-04-01 The Library After School
The 'Magic Academy library was dead silent. Soft afternoon sunlight filtered through the gaps between bookshelves that reached up to the high ceiling.
In a corner scented with aged parchment, 'Ars had a thick grimoire open before him.

@01-04-02 Confrontation in the Library
"I knew you'd be here, 'Ars."@Lilia

With an exasperated voice, 'Lilia pulled up an adjacent chair.
Her neatly brushed silver hair swayed as she peered down at the book in 'Ars's hands.

"You know final exams start tomorrow, right? The 'Headmaster said anyone with failing marks this time will be stuck in remedial classes all break."@Lilia
;
"I know. But take a look at this."@Ars

'Ars pointed his fingertip toward an intricate magic circle drawn on the parchment.

"It's a blueprint for the 'Stardust Clock—the one said to be sleeping in the underground ruins of the 'Old School Building. If this is real, we can prove the existence of lost ancient alchemy."@Ars
;
"Not another one of those dubious rumors..."@Lilia

'Lilia let out a sigh, but as her eyes were drawn to the blueprint's exquisite detail, the words died in her throat.
'Ars did not miss the small spark of curiosity igniting in her blue eyes.
;
"...So, do you really plan on going there?"@Lilia
"Today after school, right when the 'Old School Building is left unlocked. You're coming too, aren't you, 'Lilia?"@Ars
"...I'll come along. I can't have you getting lost, after all."@Lilia

Saying so, 'Lilia looked away, the corners of her lips curling upward ever so slightly.
```

The "novnow format" is designed to leave no room for ambiguity on the reader's part.

The novnow format includes a feature to export your novel as plain text data. All format-specific symbols can be stripped away with a single click.

In the future, following continued research and development, we plan to implement export features for various media beyond novels, such as visual novels and 3D CG animation.

## Summary of Editor Specifications

1. **Focused Writing Centered on 1 Section as the Smallest Unit (`write` workspace)**
   - The "Section Editor" in the lower pane allows you to focus solely on the section currently being written, while the "Context View" in the upper pane displays up to 3 preceding and following sections in read-only mode.
   - The "Reference Pane" on the right automatically extracts and displays the plot, characters (active modes/role-language), terms, reference materials, and diagnostics relevant to the current section.
2. **Lightweight Proprietary Text Syntax and Lossless Saving**
   - Manuscript text (`contents/AA-BB.txt`), overall and chapter plots (`plots/all.txt`, `plots/AA.txt`), episode plots (`plots/AA-BB.txt`), characters (`characters.md`), terms (`words.md`), ideas (`ideas.md`), and materials (`materials/`) are all managed as plain text (UTF-8), allowing native version control and diffing via Git.
3. **Semantic Resolution Engine and 76 Types of Real-Time Diagnostics**
   - Provides glossary resolution via `'Name` (shortest-match, NFKC normalization), context-aware character mode resolution, role-language pattern matching (visualizing tone-matching in dialogue with green underlines), detection of term usage prior to initial introduction, and authoring assistance diagnostics for sentence length, consecutive sentence endings, spelling inconsistencies, and more.
4. **Keyboard-Driven 8 Workspaces and Unified Palette**
   - Instantly switch between manuscript (`write`), episode plot (`plot-episode`), outline plot (`plot-outline`), ideas (`idea`), materials (`material`), casting (`cast`), agents (`agent`), and review (`review`) using `Alt+1` through `Alt+9`.
   - Jump to any element in the project via the unified palette (`Ctrl+P`) using prefix characters (`@` structure, `'` dictionary, `#` materials, `>` commands).
5. **Human-Controlled AI Agent Integration (MCP Compatible)**
   - Configure 4 delegation levels (`off` / `propose` / `apply` / `autonomous`) per role (`writer`, `plotter`, `ideator`, `researcher`, `stylist`, `logician`, `integrator`), supporting diff reviews of proposed changes and MCP server integration via `novnow mcp`.
6. **Annotation-Stripped Export**
   - Strips authoring structural headers, comments, `'` references, and material tags to output plain text (`.txt`) or Markdown (`.md`) tailored for web publishing platforms and vertical text viewers.

---

## About the Specifications

All specifications for the editor and the novnow format are fully documented:

- [`overview.md`](./docs/specifications/overview.md): Design principles, terminology, and architecture
- [`project.md`](./docs/specifications/project.md): Project structure, `project.json`, and sample layouts
- [`syntax.md`](./docs/specifications/syntax.md): Syntax specifications for each file
- [`semantics.md`](./docs/specifications/semantics.md): Dictionaries, shortest-match, mode resolution, and role-language pattern language
- [`diagnostics.md`](./docs/specifications/diagnostics.md): Diagnostics catalog
- [`editing.md`](./docs/specifications/editing.md): Section-based editing, auto-save, external change conflicts, and change proposals
- [`ui.md`](./docs/specifications/ui.md): Screen specifications, workspaces, themes, and internationalization
- [`platform.md`](./docs/specifications/platform.md): Storage boundaries, Web IndexedDB, and CLI startup specifications
- [`agent.md`](./docs/specifications/agent.md): AI agents, delegation levels, MCP, and prompt specifications
- [`workflows.md`](./docs/specifications/workflows.md): Material collection, export, and performance/quality requirements

You can also view them on our official website ([https://novnow.sonneko.com/specifications/](https://novnow.sonneko.com/specifications/)).

## About the License

novnow is free for anyone to use. It will never become a paid service.

Both the "novnow format" specifications and the "novnow editor" source code are freely available under the `MIT License`.
