# Navigation, sorting and search

Each tab owns its criteria, results and cancellation. Workers prepare data; publication checks the tab's identity and the request's revision. An old response must never replace a more recent navigation.

Duplicate results use a virtualized list shared by group headers and files. Filter, min/max sizes and row ranges are prepared in the background; collapsing a group invalidates the previous preparation. Rendering walks only the rows the viewport asks for, even for a group of 100,000 files. A common size shows if all members share it; otherwise the group shows its size range.

## Listing and filters

Folders stay first in ascending and descending order. The UI sorts names in natural order, in Unicode lowercase without stripping accents: `file2` comes before `file10`. ASCII digit runs compare with no size limit; at equal value, fewer leading zeros come first (`2`, `02`, `002`). Extensions stay lexicographic, with names as natural tiebreak for all columns and stable ties. The backend's public natural sort keeps its other consumers.

Name and location use a case-insensitive, accent-sensitive substring. Prepare the query once per traversal. Hidden/system visibility must stay consistent between listing and search.

Date grouping belongs to the tab and is persisted; collapsing stays temporary. The six calendar periods are shared by filters and groups. A day/timezone change invalidates caches in the background, inactive tabs included, without rereading files. Groups use the same geometry for rendering and selection.

## Folder sizes

The calculation gives an indicative size of accessible items, suited to a file explorer. A disk analyzer's exhaustiveness is not a goal: unreadable items may be omitted without asking for more rights or showing an incompleteness status. Watchers and deferred recalculation improve freshness, with no guarantee of exhaustive measure or snapshot. This limit is an accepted product decision.

The status bar button turns the calculation on for the tab's path until its next navigation. A refresh keeps non-invalidated sizes and only restarts folders flagged, unmeasured, or whose invalidated measure failed. A traversal already running or ready to publish is not duplicated if no new change arrived. The refreshed listing adds new folders to measure. Watching becomes recursive during this mode; a deep change recalculates only the immediate subfolder concerned. Symbolic links and junctions are excluded from the calculation, virtual views and archives from this mode.

Events are grouped by root and child, falling back to full invalidation on overflow or lost events. Refresh also accounts for events received before the 500 ms periodic processing, without reapplying them twice or losing listing requests or other tabs' changes on the same root. Keeping a size without a new traversal requires an error-free native recursive watcher; without one, during installation, after an error or with polling, a refresh reinvalidates all sizes as a precaution.

Watching stays active during the first pass to remember changes. The queue keeps at most one pending request per folder; a change during its traversal or before publication keeps the shown measure and adds one new pass. A value to refresh only changes color (the active theme's `accent`), with no prefix or font/width change. In the grid, the folder's type label uses the same color. The three explanatory sentences appear on three lines in a frame inside the file's normal tooltip: accent border and text, slightly tinted accent background. Width calculation includes the frame's margins and border. No tooltip nests on the size or label. The shared hover session alone handles showing and closing. Normal color returns after a traversal publishes with no new change.

Explicit activation asks for the recursive watcher before starting first measures, then starts with no fixed delay. A logged installation failure lets the initial calculation start. Installation does not reinvalidate intact initial requests; replacing a watcher keeps a cautious check for measures already started or available. Initial queuing alone does not mark a size provisional. The tooltip says a refresh is running or pending, which may also come from a cautious check, without claiming a change was seen.

Watch events group over 500 ms. Folders with no first measure go before eligible recalculations of the same tab. After a measure, an automatic recalculation waits at least five seconds after the traversal ends and five seconds with no new event. Continuous activity thus defers recalculation, keeping the last shown value: large active trees are no longer rescanned in a loop. The first measure stays publishable even under continuous writes. Refreshing or re-clicking the sizes button forces the needed requests without waiting, keeping up-to-date sizes; later events neither push back this explicit request nor turn later passes into forced recalculations.

A finished result awaits publication before another traversal of the same folder can start. The scheduler stays active during these waits, holding no physical slot, and fills two slots without waiting for slower folders or display preparation; the engine keeps its global limit of two physical traversals, even after cancel. Changes during a listing ask for one extra listing. Native watching stays event-driven. Polling media keep two seconds for non-recursive listings; their recursive snapshots are thirty seconds apart, since each walks the subtree. Their initial snapshot and periodic traversals cost disk time and detect deep changes less immediately.

The general Turbo preference also applies to Properties. Windows priority follows the mode during traversal; the budget is chosen at the next traversal without restarting running work. Each SSD traversal uses up to 2 workers without Turbo and up to 8 with it, capped by logical processors; HDD, network or unknown storage stay at 1 worker per traversal. Pools are reused, independent between both traversals, and their threads return to normal priority at the end. Counters aggregate per directory before merging. Results wait in a separate queue and publish in batches of at most 128, with 50 ms grouping before each batch; their revisions are rechecked before publication. A preparation gone stale is retried on the still-valid subset, without blocking on a changed folder. Exclusions `System Volume Information` and `Microsoft\Windows\Containers` stay active in both modes, on full path components, case-insensitively.

Each result checks tab identity, path, mode generation and folder revision before publication. Sorting, filters and column summaries are prepared in the background. Navigation cancels the traversal and empties the queue, also toward a search or after a disk is removed. Watch errors are logged; a manual refresh can always restart the calculation. The result stays a measure of the traversal, with no snapshot guarantee against concurrent writes.

## Search paths

| Path | Contract |
| --- | --- |
| Path completion | Up to twelve folders, read deferred by 150 ms; Up/Down, Tab and click keep editing active; no virtual completion |
| Quick search | Recursive names, case-insensitive, `*`/`?` patterns, starts after 350 ms or Enter; neither content nor archives |
| Advanced search | Criteria kept in its window; runs in the originating panel without closing the form |
| Archive search | Names/metadata via a dedicated worker; unsupported criteria flagged, without publishing misleading matches |
| Duplicates | Name, size and content combinable; comparing content requires equal size; overlapping roots deduplicated; `$Recycle.Bin` folders excluded case-insensitively, roots inside a trash included |

Results arrive in bounded batches; Stop keeps those received. The virtual view offers neither folder creation nor paste. Open location selects and reveals the file in its folder.

## Text content: bounded memory

`commands/search/content.rs` keeps short lines up to 64 KiB, then uses an incremental Thompson automaton. Block boundaries must change neither anchors, Unicode nor matching: no truncation or approximate fixed overlap.

Validate the whole line before publishing a match; a UTF-8/I/O error or cancel invalidates it. The automaton's memory depends on the pattern. Normalizing exceptional combining sequences may use a private temporary: the RAM bound is not a disk quota.

This bound concerns plain text. Content search inside archives stays unavailable. None of these paths guarantees a global RSS cap or a snapshot against concurrent changes.

## Office content search

`Platform::office_search_text` produces whole text within its scope or an explicit error. On Windows, a dedicated mode of `oxyde-image-worker` receives only the read source handle and its extension. It runs in capability-less LPAC, with the existing supervisor and aggregate admission, and no local fallback if the worker is missing, failing or late. Its admission is separate from Office previews; cancel is checked during waiting and supervision.

Limits: encoded source 128 MiB, 10,000 entries, 1 MiB of cumulative names, 16 MiB per XML part and 64 MiB of cumulative XML, 8 MiB of response text, 512 MiB job and a 15 s timeout including admission waiting. The parent does not decompress the document and validates a bounded text response before using the streaming search engine. Linux uses the same extractor in a [confined worker](linux-previews.md#images-office-and-audio), without LPAC guarantees.

The Word body, slide parts and sheet cells are walked with no arbitrary limit to the first three slides or 64 columns. Inline cells and shared string references are supported; XML entities share their decoding with the Viewer. Search computes neither formulas nor number formats and ignores OpenDocument presentation notes, like the Viewer's main text.

A decompression or output limit, an unreadable document or a worker failure produces `ContentSkipped`: the banner says in all five languages that some content was not verified and gives the first path concerned. No Office truncation is presented as a successful full search.
