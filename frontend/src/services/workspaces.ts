export interface WorkspaceDefinition {
  id: "spectrum" | "threats" | "investigation" | "sigint" | "devices";
  href: string;
  label: string;
  shortcut: "1" | "2" | "3" | "4" | "5";
}

export const WORKSPACES: WorkspaceDefinition[] = [
  { id: "spectrum", href: "/", label: "Spectrum", shortcut: "1" },
  { id: "threats", href: "/threats", label: "Threats", shortcut: "2" },
  {
    id: "investigation",
    href: "/investigation",
    label: "Investigation",
    shortcut: "3",
  },
  { id: "sigint", href: "/sigint", label: "SIGINT", shortcut: "4" },
  { id: "devices", href: "/device", label: "Devices", shortcut: "5" },
];

export function workspaceFromShortcut(key: string) {
  return WORKSPACES.find((workspace) => workspace.shortcut === key);
}
