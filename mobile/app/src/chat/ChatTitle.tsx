import { StyleSheet, Text } from 'react-native';

import { AgentIcon } from '@/ui/Icon';
import { fonts, type Palette, useStyles } from '@/ui/theme';

/** A chat's header: its agent's mark, its title, then its name dimmed, as the Mac's header reads; the same before and after the chat starts. */
export function ChatTitle({ provider, name, title }: { provider: string; name?: string; title: string }) {
  const styles = useStyles(makeStyles);
  return (
    <>
      <AgentIcon provider={provider} size={17} />
      <Text style={styles.title} numberOfLines={1} accessibilityRole="header">
        {title}
      </Text>
      {name ? <Text style={styles.name}>{name}</Text> : null}
    </>
  );
}

const makeStyles = (colors: Palette) => {
  return StyleSheet.create({
    title: { fontFamily: fonts.uiSemibold, fontSize: 16, letterSpacing: -0.25, color: colors.ink, flexShrink: 1 },
    name: { fontFamily: fonts.ui, fontSize: 12, color: colors.tertiary },
  });
};
