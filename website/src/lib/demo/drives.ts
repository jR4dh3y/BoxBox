// The sample drives on the preview's This Server page, shown as cards or as a list.

export type Drive = { path: string; total: string; used: string; free: string; device: string; fs: string; percent: number };

export const drives: readonly Drive[] = [
  { path: 'root', total: '217.97 GB', used: '105.22 GB', free: '101.61 GB', device: '/dev/sda2', fs: 'ext4', percent: 48.3 },
  { path: 'storage', total: '223.57 GB', used: '211.18 GB', free: '12.39 GB', device: '/dev/sdc1', fs: 'fuseblk', percent: 94.5 },
  { path: 'archive', total: '101.56 TB', used: '61.24 TB', free: '40.32 TB', device: 'archive_remote:', fs: 'fuse.rclone', percent: 60.3 },
  { path: 'backups', total: '5 TB', used: '59.67 GB', free: '4.94 TB', device: 'backups_remote:', fs: 'fuse.rclone', percent: 1.2 },
  { path: 'home', total: '217.97 GB', used: '105.22 GB', free: '101.61 GB', device: '/dev/sda2', fs: 'ext4', percent: 48.3 }
];

/** The app's usage levels (frontend/src/lib/utils/driveUsage.ts): fixed colours, not the accent. */
export function usageClasses(percent: number): { bar: string; text: string } {
  if (percent >= 90) return { bar: 'bg-danger', text: 'text-danger' };
  if (percent >= 75) return { bar: 'bg-warning', text: 'text-warning' };
  return { bar: 'bg-info', text: 'text-text-muted' };
}
