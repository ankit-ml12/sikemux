import { basename } from "../lib/paths";
import type { QueuedMessage } from "./types";

export const queuedLabel = (message: QueuedMessage): string =>
    message.text || [...message.paths.map(basename), ...message.context.map((item) => item.title)].join(", ");
