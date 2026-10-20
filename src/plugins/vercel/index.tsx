import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { vercelStatusR } from "./resources";
import { VercelMark } from "./components/VercelMark";
import { VERCEL_PLUGIN, VERCEL_PROJECTS } from "./kinds";
import { openVercel } from "./state";

const VercelPane = lazy(() => import("./components/VercelPane").then((module) => ({ default: module.VercelPane })));

registerFrontendPlugin({
    id: VERCEL_PLUGIN.id,
    surfaces: [
        {
            kind: VERCEL_PROJECTS,
            title: VERCEL_PLUGIN.name,
            icon: (size) => <VercelMark size={size} className="icon-vercel" />,
            render: ({ paneId, visible }) => <VercelPane paneId={paneId} active={visible} />,
        },
    ],
    open: openVercel,
    openTitle: `Open ${VERCEL_PLUGIN.name}`,
    mark: (size) => <VercelMark size={size} className="icon-vercel" />,
    linkHosts: ["vercel.com"],
    useSignIn: function useServiceSignIn() {
        return signInOf(useResource(vercelStatusR));
    },
});
