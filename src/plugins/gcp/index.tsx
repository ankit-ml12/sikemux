import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { gcpStatusR } from "./resources";
import { GcpMark } from "./components/icons";
import { GCP_CONSOLE, GCP_PLUGIN_ID } from "./kinds";
import { openGcp } from "./state";

const GcpPane = lazy(() => import("./components/GcpPane").then((module) => ({ default: module.GcpPane })));

registerFrontendPlugin({
    id: GCP_PLUGIN_ID,
    surfaces: [
        {
            kind: GCP_CONSOLE,
            title: "Google Cloud",
            icon: (size) => <GcpMark size={size} />,
            render: ({ paneId, visible }) => <GcpPane paneId={paneId} active={visible} />,
        },
    ],
    open: openGcp,
    openTitle: "Open Google Cloud",
    mark: (size) => <GcpMark size={size} />,
    linkHosts: ["console.cloud.google.com"],
    useSignIn: function useServiceSignIn() {
        const status = useResource(gcpStatusR);
        return signInOf({
            ...status,
            data: status.data && { configured: status.data.installed && status.data.accounts.length > 0, authFailed: false },
        });
    },
});
