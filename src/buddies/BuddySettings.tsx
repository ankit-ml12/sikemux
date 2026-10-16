import { useMemo } from "react";
import { SettingsRow, SettingsRows, SettingsSection } from "../settings/SettingsLayout";
import * as cmd from "../state/commands";
import { useStore } from "../state/store";
import { Switch } from "../ui/Controls";
import { Dropdown } from "../ui/Dropdown";
import { BuddyArt } from "./Buddy";
import { BUDDY_PACKS, EVERY_PACK, EVERY_SHAPE, packById, resolveBuddy } from "./catalogue";

const SAMPLES = Array.from({ length: 8 }, (_, color) => ({ id: `sample-${color}`, buddy: { color } }));

export function BuddySettings() {
    const pack = useStore((s) => s.buddyPack);
    const shape = useStore((s) => s.buddyShape);
    const motion = useStore((s) => s.buddyMotion);
    const chosen = packById(pack);
    const samples = useMemo(() => SAMPLES.map((sample) => resolveBuddy(sample, { pack, shape })), [pack, shape]);

    return (
        <SettingsSection title="Buddies">
            <SettingsRows>
                <SettingsRow label="Pack" desc="The characters new agents get. Right-click an agent to give it a different one.">
                    <Dropdown
                        className="settings-dd"
                        label="Buddy pack"
                        value={pack}
                        options={[
                            ...BUDDY_PACKS.map((candidate) => ({ value: candidate.id, label: candidate.name })),
                            { value: EVERY_PACK, label: "Every pack" },
                        ]}
                        onChange={(value) => cmd.setBuddyStyle(value, EVERY_SHAPE)}
                    />
                </SettingsRow>
                {chosen && (
                    <SettingsRow label="Shape" desc="One shape for every agent, or a different one each.">
                        <Dropdown
                            className="settings-dd"
                            label="Buddy shape"
                            value={shape}
                            options={[
                                { value: EVERY_SHAPE, label: "A different one each" },
                                ...chosen.shapes.map((candidate) => ({ value: candidate.id, label: candidate.name })),
                            ]}
                            onChange={(value) => cmd.setBuddyStyle(pack, value)}
                        />
                    </SettingsRow>
                )}
                <SettingsRow label="Preview">
                    <div className="buddy-samples">
                        {samples.map((look, index) => (
                            <BuddyArt key={index} pack={look.pack} shape={look.shape} color={look.color} state="idle" size={28} />
                        ))}
                    </div>
                </SettingsRow>
                <SettingsRow
                    label="Animate faces"
                    desc="Buddies blink, look around and react to what their agent is doing."
                    asLabel
                    control={<Switch checked={motion} onChange={cmd.setBuddyMotion} label="Animate faces" />}
                />
            </SettingsRows>
        </SettingsSection>
    );
}
