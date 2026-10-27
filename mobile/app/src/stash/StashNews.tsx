import { Pressable, StyleSheet, Text, View } from 'react-native';

import { haptics } from '@/ui/haptics';
import { Icon } from '@/ui/Icon';
import { fonts, type Palette, useColors, useStyles } from '@/ui/theme';
import type { StashItem } from './api';
import { newInStash, seeStash, useStash } from './store';

/** "N new in Stash" above the composer while something was stashed since the person last looked; Add attaches them. */
export function StashNews({ onAdd }: { onAdd: (items: StashItem[]) => void }) {
  const colors = useColors();
  const styles = useStyles(makeStyles);
  const fresh = newInStash(useStash());
  if (!fresh.length) return null;
  return (
    <View style={styles.chip}>
      <Icon name="IconStack" size={13} color={colors.inkDim} />
      <Text style={styles.text} numberOfLines={1}>
        <Text style={styles.count}>{fresh.length}</Text> new in Stash
      </Text>
      <Pressable
        onPress={() => {
          haptics.select();
          onAdd(fresh);
          void seeStash();
        }}
        hitSlop={6}
        style={({ pressed }) => [styles.add, pressed && { backgroundColor: colors.active }]}
        accessibilityRole="button"
        accessibilityLabel={`Add ${fresh.length} new from the stash`}>
        <Text style={styles.addText}>Add</Text>
      </Pressable>
      <Pressable onPress={() => void seeStash()} hitSlop={8} style={styles.close} accessibilityRole="button" accessibilityLabel="Dismiss">
        <Icon name="IconClose" size={11} color={colors.tertiary} />
      </Pressable>
    </View>
  );
}

const makeStyles = (colors: Palette) =>
  StyleSheet.create({
    chip: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: 8,
      alignSelf: 'flex-start',
      height: 36,
      marginBottom: 8,
      paddingLeft: 12,
      paddingRight: 4,
      borderRadius: 10,
      borderWidth: 1,
      borderColor: colors.border,
      backgroundColor: colors.overlay,
    },
    text: { flexShrink: 1, fontFamily: fonts.ui, fontSize: 13, color: colors.secondary },
    count: { fontFamily: fonts.uiMedium, color: colors.ink, fontVariant: ['tabular-nums'] },
    add: { height: 26, paddingHorizontal: 10, borderRadius: 7, alignItems: 'center', justifyContent: 'center' },
    addText: { fontFamily: fonts.uiSemibold, fontSize: 13, color: colors.accent },
    close: { width: 26, height: 26, alignItems: 'center', justifyContent: 'center' },
  });
