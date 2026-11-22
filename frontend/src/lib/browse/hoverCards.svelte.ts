/**
 * The file and drive hover cards. Their code loads when the pointer first reaches an item, so a
 * page that is only clicked through never downloads it; until it arrives, this keeps where the
 * pointer is.
 */
import type { FileInfo } from '#lib/api/files.js';
import type {
	DriveHover,
	DriveHoverOptions,
	HoverCard,
	HoverCardOptions
} from './hoverCard.svelte.js';
import type { HoverPoint, HoverTarget } from './hoverPoint.js';

type Resting =
	| { kind: 'file'; item: FileInfo; at: HoverPoint }
	| { kind: 'drive'; path: string; name: string; at: HoverPoint };

export type HoverCardsOptions = HoverCardOptions & Pick<DriveHoverOptions, 'stats'>;

export class HoverCards {
	cards = $state.raw<{ file: HoverCard; drive: DriveHover }>();
	/** The cards' code would not download; views keep their native tooltips instead. */
	failed = $state(false);
	private resting: Resting | null = null;
	private readonly options: HoverCardsOptions;

	constructor(options: HoverCardsOptions) {
		this.options = options;
	}

	/** What rows and tiles report to. */
	readonly files: HoverTarget = {
		rest: (item, at) => this.point({ kind: 'file', item, at }),
		hide: () => this.hide()
	};

	drive(path: string, name: string, at: HoverPoint): void {
		this.point({ kind: 'drive', path, name, at });
	}

	/** The pointer left, pressed, or a menu, drag or dialog covered what it rested on. */
	hide(): void {
		this.resting = null;
		this.cards?.file.hide();
		this.cards?.drive.hide();
	}

	private point(resting: Resting): void {
		this.resting = resting;
		if (this.cards) return show(this.cards, resting);
		if (this.failed || resting.at.touch || !this.options.enabled()) return;
		import('./hoverCard.svelte.js').then(
			({ HoverCard, DriveHover }) => {
				this.cards ??= { file: new HoverCard(this.options), drive: new DriveHover(this.options) };
				// The pointer may have left while the code loaded.
				if (this.resting) show(this.cards, this.resting);
			},
			// A browser keeps a failed import, so trying again before a reload would not help.
			() => (this.failed = true)
		);
	}
}

function show(cards: { file: HoverCard; drive: DriveHover }, resting: Resting): void {
	if (resting.kind === 'file') {
		cards.drive.hide();
		cards.file.rest(resting.item, resting.at);
	} else {
		cards.file.hide();
		cards.drive.rest(resting.path, resting.name, resting.at);
	}
}
