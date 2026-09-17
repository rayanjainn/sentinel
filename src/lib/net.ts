// Network display helpers shared by the process drawer and the Network view.
import type { IpFamily } from "../bindings/IpFamily";
import type { TcpState } from "../bindings/TcpState";

export const TCP_STATE_LABEL: Record<TcpState, string> = {
  listen: "Listening",
  synSent: "Opening",
  synReceived: "Opening",
  established: "Established",
  finWait1: "Closing",
  finWait2: "Closing",
  closeWait: "Close wait",
  closing: "Closing",
  lastAck: "Closing",
  timeWait: "Time wait",
  closed: "Closed",
  deleteTcb: "Closed",
  unknown: "Unknown",
};

/** "192.168.1.4:443", "[2606:4700::6810]:443", "*:5353" for unspecified addresses. */
export function formatEndpoint(addr: string | null, port: number | null, family: IpFamily): string {
  const host = addr === null ? "*" : addr === "0.0.0.0" || addr === "::" ? "*" : addr;
  const bracketed = family === "v6" && host !== "*" && host.includes(":") ? `[${host}]` : host;
  return port === null || port === 0 ? bracketed : `${bracketed}:${port}`;
}
