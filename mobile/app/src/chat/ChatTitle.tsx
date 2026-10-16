import { StyleSheet, Text } from 'react-native';

import { AgentIcon } from '@/ui/Icon';
import { fonts, type Palette, useStyles } from '@/ui/theme';

/** A chat's header: its agent's mark, then its name and title, the same before and after the chat starts. */
export function ChatTitle({ provider, name, title }: { provider: string; name?: string; title: string }) {
  const styles = useStyles(makeStyles);
  return (
    <>
      <AgentIcon provider={provider} size={17} />
      <Text style={styles.title} numberOfLines={1} accessibilityRole="header">
        {name ? (
          <>
            {name}
            <Text style={styles.titleDot}> · </Text>
            <Text style={styles.titleAfterName}>{title}</Text>
          </>
        ) : (
          title
        )}
      </Text>
    </>
  );
}

const makeStyles = (colors: Palette) => {
  return StyleSheet.create({
    title: { fontFamily: fonts.uiSemibold, fontSize: 16, letterSpacing: -0.25, color: colors.ink, flexShrink: 1 },
    titleDot: { color: colors.inkFaint },
    titleAfterName: { fontFamily: fonts.uiMedium, color: colors.tertiary },
  });
};
