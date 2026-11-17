// The closing section writes the visitor's install command from three words
// they can change. What they type ends up in a shell, so each word is parsed
// into something that is safe there, or rejected. public/install.sh applies
// the same rules again to whatever it is given.

export type InstallChoices = { folder: string; port: number; user: string };

export const defaultChoices: InstallChoices = { folder: '/home/you', port: 8080, user: 'admin' };

const REPOSITORY = 'https://github.com/jR4dh3y/BoxBox';

/** Served from website/public/install.sh. */
export const SCRIPT_URL = 'https://boxbox.radhey.dev/install.sh';

/** An absolute host path made only of characters that need no quoting. */
export function parseFolder(text: string): string | null {
  const value = text.trim();
  return /^\/[A-Za-z0-9._/-]*$/.test(value) && !value.split('/').includes('..') ? value : null;
}

export function parsePort(text: string): number | null {
  const value = text.trim();
  if (!/^[0-9]{1,5}$/.test(value)) return null;
  const port = Number(value);
  return port >= 1 && port <= 65535 ? port : null;
}

/** BoxBox reads users from BOXBOX_USERS_<name>, so the name must fit there. */
export function parseUser(text: string): string | null {
  const value = text.trim();
  return /^[A-Za-z][A-Za-z0-9_]{0,31}$/.test(value) ? value : null;
}

/** `chosen` text is one of the visitor's own words. */
export type CommandPart = { text: string; chosen: boolean };

/** The one command that runs the setup script, in pieces the page can colour. */
export function installCommandParts({ folder, port, user }: InstallChoices): CommandPart[] {
  const fixed = (text: string): CommandPart => ({ text, chosen: false });
  const chosen = (text: string): CommandPart => ({ text, chosen: true });
  return [
    fixed(`curl -fsSL ${SCRIPT_URL} | bash -s -- \\\n  --folder `),
    chosen(folder),
    fixed(` \\\n  --port `),
    chosen(String(port)),
    fixed(` \\\n  --user `),
    chosen(user)
  ];
}

export function installCommand(choices: InstallChoices): string {
  return installCommandParts(choices).map((part) => part.text).join('');
}

/**
 * What a visitor hands to a coding agent instead of running the command. It
 * carries no settings on purpose: the agent is told to inspect the machine,
 * ask, and follow the project's own docs.
 */
export const agentPrompt = `I want to run BoxBox, a self-hosted web file manager, on this machine. Please set it up for me.

Project: ${REPOSITORY}
Read README.md, docs/quickstart.md, docs/docker.md and docs/configuration.md there before you start. They are the source of truth; follow them over anything you remember.

1. Look before you act. Find out the OS and CPU architecture, whether Docker and its compose plugin are installed and running, whether I can use Docker without sudo, my user and group IDs, which ports are already in use, which disks and folders hold my files (home folder, mounted drives, network shares), and whether a reverse proxy or other containers already run here.

2. Pull the latest released image, ghcr.io/jr4dh3y/boxbox:latest, and check ${REPOSITORY}/releases so you can tell me which version it is.

3. Ask me, in one short list, only what you cannot work out yourself. Suggest a default for each answer based on what you found:
   - which folders BoxBox should show, and whether any should be read-only
   - which port to use (suggest one that is free)
   - the user name or names
   - whether it should be reachable only from this machine, from my network, or through a domain
   - a password for each user

4. Set it up with Docker Compose as the docs describe. Generate a random JWT secret, store each password only as a bcrypt hash, mount only the folders I chose, and keep the container unprivileged as the project's compose file does. Check that the user the container runs as can read the folders I chose; if it cannot, explain the options and let me decide.

5. Start it. Confirm the container is healthy and that http://localhost:<port>/health answers. Then tell me the address, the user name, where the configuration lives, and how to update and remove it.

Rules: never print or log a password or the secret. Do not install system packages, open firewall ports, or change file permissions without asking me first. If a step fails, show me the error instead of guessing.`;
