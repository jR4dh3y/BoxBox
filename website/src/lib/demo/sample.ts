// The sample server the preview opens: a few drives with believable homelab
// files. Every instance of the preview gets its own copy to change.

import type { Entry, Folder, TextFile } from './fs';
import { picture } from './pictures';

/** Mounts the preview treats as read-only, like `read_only: true` mounts. */
export const readOnlyMounts: ReadonlySet<string> = new Set(['backups']);

function folder(name: string, children: Entry[] = []): Folder {
  return { kind: 'folder', name, children };
}

function text(name: string, content: string): TextFile {
  return { kind: 'text', name, content };
}

const compose = `services:
  boxbox:
    image: ghcr.io/jr4dh3y/boxbox:v0.3.2
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - /srv/media:/media/media
      - boxbox-data:/data

  jellyfin:
    image: jellyfin/jellyfin:latest
    ports:
      - "8096:8096"
    volumes:
      - /srv/media:/media:ro

volumes:
  boxbox-data:
`;

const photos = ['alps', 'aurora', 'canyon', 'coast', 'desert', 'dusk', 'forest', 'glacier', 'harbor', 'lake', 'meadow', 'ridge', 'sunset'];

export function createSampleFs(): Folder {
  return folder('', [
    folder('root', [
      folder('etc', [
        text('fstab', '# <device>  <dir>          <type>  <options>  <dump> <fsck>\n/dev/sda2   /             ext4    defaults   0      1\n/dev/sdc1   /mnt/storage  ntfs-3g defaults   0      0\n'),
        text('hosts', '127.0.0.1   localhost\n192.168.1.20  nas.lan\n192.168.1.21  jellyfin.lan\n')
      ]),
      folder('srv', [folder('media')])
    ]),
    folder('storage', [
      folder('movies', [text('watchlist.md', '# Watchlist\n\n- [ ] Rush (2013)\n- [x] Senna (2010)\n- [ ] Ford v Ferrari (2019)\n')]),
      folder('photos', [picture('city.jpg'), picture('mountains.jpg'), picture('ocean.jpg')]),
      folder('stacks', [text('docker-compose.yml', compose)])
    ]),
    folder('archive', [
      folder('2024', [picture('winter.jpg'), picture('spring.jpg')]),
      folder('2025', [picture('roadtrip.jpg'), text('index.md', '# 2025\n\nScans and exports, sorted by month.\n')])
    ]),
    folder('backups', [
      text('nightly.log', '02:00:01 snapshot started\n02:03:44 59.67 GB in 18,204 files\n02:03:45 snapshot 7f3a91c saved\n02:03:45 pruned 2 snapshots older than 30 days\n'),
      text('restic.conf', 'repository: rclone:backups_remote:nas\nschedule: daily\nkeep-daily: 30\n')
    ]),
    folder('home', [
      folder('Desktop', [text('todo.txt', 'label the new drives\nrenew the domain\nmove photos off the phone\n')]),
      folder('Downloads', [picture('wallpaper.jpg'), text('release-notes.md', '# BoxBox v0.3.2\n\nSecurity and desktop pairing fixes.\n')]),
      folder('Documents', [
        text('Caddyfile', 'files.example.com {\n\treverse_proxy localhost:8080\n}\n'),
        text('docker-compose.yml', compose),
        text('notes.md', '# Homelab notes\n\n- BoxBox on :8080\n- Jellyfin on :8096\n- Backups run at 02:00\n')
      ]),
      folder('Music', [text('playlist.m3u', '#EXTM3U\nlofi/01-rain.flac\nlofi/02-night-drive.flac\n')]),
      folder('Pictures', [
        folder('2026', [folder('summer-trip', ['coast-797.jpg', 'harbor-411.jpg', 'ocean-235.jpg', 'sunset-758.jpg'].map(picture))]),
        ...photos.map((name) => picture(`${name}.jpg`))
      ]),
      folder('Videos')
    ])
  ]);
}
