import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { rndStatusR } from "./resources";
import { IconRundeck } from "../../plugin-api/ui";
import { RundeckOverlay } from "./components/RundeckOverlay";
import { RundeckTopBarItem } from "./components/RundeckTopBarItem";
import { RUNDECK_DEPLOY, RUNDECK_PLUGIN } from "./kinds";
import { openRundeckSession, toggleRundeckJobPalette } from "./state";

const RundeckPane = lazy(() => import("./components/RundeckPane").then((module) => ({ default: module.RundeckPane })));

registerFrontendPlugin({
    id: RUNDECK_PLUGIN.id,
    surfaces: [
        {
            kind: RUNDECK_DEPLOY,
            title: RUNDECK_PLUGIN.name,
            icon: (size) => <IconRundeck size={size} />,
            render: ({ paneId, visible }) => <RundeckPane paneId={paneId} active={visible} />,
            quickOpen: toggleRundeckJobPalette,
        },
    ],
    open: openRundeckSession,
    openTitle: `Open ${RUNDECK_PLUGIN.name} deploy center`,
    mark: (size) => <IconRundeck size={size} />,
    Overlay: RundeckOverlay,
    TopBarItem: RundeckTopBarItem,
    useSignIn: function useRundeckSignIn() {
        const status = useResource(rndStatusR);
        return signInOf(status);
    },
});
