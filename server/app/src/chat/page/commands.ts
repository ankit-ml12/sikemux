/*
 * What the chat view asks of the app around it. A shared chat's files are on its sharer's
 * computer, so they open nowhere; a web page opens in a tab.
 */
export interface FilesPlace {
  cwd: string;
  host?: string;
}

export function openFileOnDesk(
  _agentId: string,
  _path: string,
  _line?: number,
  _column?: number,
) {}

export function requestOpenFile(
  _path: string,
  _line?: number,
  _column?: number,
  _place?: FilesPlace,
) {}
