export type DeploymentIcon = 'server' | 'cloud' | 'home' | 'building' | 'hard-drive';
export const deploymentIcons: { value: DeploymentIcon; label: string }[] = [
	{ value: 'server', label: 'Server' },
	{ value: 'cloud', label: 'Cloud' },
	{ value: 'home', label: 'Home' },
	{ value: 'building', label: 'Office' },
	{ value: 'hard-drive', label: 'Drive' }
];
export function parseDeploymentIcon(value: unknown): DeploymentIcon {
	if (value === undefined || value === '') return 'server';
	const icon = deploymentIcons.find((icon) => icon.value === value);
	if (!icon) throw new Error('Invalid deployment icon');
	return icon.value;
}
