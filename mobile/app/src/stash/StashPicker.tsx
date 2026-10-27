import { useEffect, useState } from 'react';
import { Alert, Image, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';

import { apiUrl } from '@/account/config';
import { haptics } from '@/ui/haptics';
import { Icon } from '@/ui/Icon';
import type { IconName } from '@/ui/icons.generated';
import { Chip } from '@/ui/controls';
import { alertFailure, PromptGlyph } from '@/ui/prompt';
import { fonts, type Palette, useColors, useStyles } from '@/ui/theme';
import type { StashItem } from './api';
import { refreshStash, removeStash, seeStash, stashToken, useStash } from './store';
import { itemPlace, itemTitle } from './words';

const GHOSTS = 3;
/** How far the list is pulled down past its top before it goes back to the tiles. */
const PULL_DOWN = 70;

/** The stash, newest first, to pick items from for the next message; `chosen` are numbered in the order picked. */
export function StashPicker({
  chosen,
  onToggle,
  onBack,
  onClose,
}: {
  chosen: readonly StashItem[];
  onToggle: (item: StashItem) => void;
  /** Pulled down from its top, back to the tiles. */
  onBack: () => void;
  onClose: () => void;
}) {
  const colors = useColors();
  const styles = useStyles(makeStyles);
  const stash = useStash();
  const bearer = useBearer();
  useEffect(() => {
    void refreshStash().then(seeStash);
  }, []);

  const remove = (item: StashItem) => {
    haptics.select();
    Alert.alert('Remove from stash?', itemTitle(item), [
      { text: 'Cancel', style: 'cancel' },
      {
        text: 'Remove',
        style: 'destructive',
        onPress: () => void removeStash(item.id).catch(alertFailure("Couldn't remove it")),
      },
    ]);
  };

  let body;
  if (stash.items.length) {
    body = (
      <ScrollView
        style={styles.list}
        contentContainerStyle={styles.listPad}
        showsVerticalScrollIndicator={false}
        onScrollEndDrag={(event) => {
          if (event.nativeEvent.contentOffset.y < -PULL_DOWN) onBack();
        }}>
        {stash.items.map((item) => (
          <Row
            key={item.id}
            item={item}
            bearer={bearer}
            order={chosen.findIndex((known) => known.id === item.id) + 1}
            onPress={() => {
              haptics.select();
              onToggle(item);
            }}
            onLongPress={() => remove(item)}
          />
        ))}
      </ScrollView>
    );
  } else if (stash.state === 'loading') {
    body = (
      <View style={styles.listPad} accessibilityLabel="Loading the stash">
        {Array.from({ length: GHOSTS }, (_, index) => (
          <View key={index} style={styles.row}>
            <View style={[styles.face, styles.ghostFace]} />
            <View style={styles.body}>
              <View style={[styles.ghostLine, { width: '64%' }]} />
              <View style={[styles.ghostLine, { width: '38%', marginTop: 8 }]} />
            </View>
          </View>
        ))}
      </View>
    );
  } else if (stash.state === 'offline') {
    body = (
      <View style={styles.empty}>
        <PromptGlyph icon="IconGlobe" small />
        <Text style={styles.emptyTitle}>{"Can't reach Sikemux"}</Text>
        <Text style={styles.emptyText}>Check the phone is online.</Text>
        <View style={{ marginTop: 10 }}>
          <Chip title="Try again" onPress={() => void refreshStash()} />
        </View>
      </View>
    );
  } else {
    body = (
      <View style={styles.empty}>
        <PromptGlyph icon="IconStack" small />
        <Text style={styles.emptyTitle}>Nothing stashed</Text>
        <Text style={styles.emptyText}>Share a link, a photo or a file to Sikemux from any app to keep it here.</Text>
      </View>
    );
  }

  return (
    <View style={styles.wrap}>
      <View style={styles.head}>
        <Pressable onPress={onClose} hitSlop={8} style={styles.close} accessibilityRole="button" accessibilityLabel="Close">
          <Icon name="IconClose" size={15} color={colors.tertiary} />
        </Pressable>
        <Text style={styles.title} accessibilityRole="header">
          Stash
        </Text>
        <View style={styles.close} />
      </View>
      {body}
      <View style={{ height: chosen.length ? 80 : 0 }} />
    </View>
  );
}

/** A sign-in for drawing stashed photos, which the server hands only to the account. */
export function useBearer(): string | undefined {
  const [bearer, setBearer] = useState<string>();
  useEffect(() => {
    stashToken()?.()
      .then((token) => setBearer(token ?? undefined))
      .catch(() => {});
  }, []);
  return bearer;
}

const KIND_ICONS: Record<StashItem['kind'], IconName> = {
  link: 'IconLink',
  text: 'IconText',
  image: 'IconImage',
  video: 'IconImage',
  file: 'IconFile',
};

export function Face({ item, bearer, size = 46 }: { item: StashItem; bearer: string | undefined; size?: number }) {
  const colors = useColors();
  const styles = useStyles(makeStyles);
  const picture =
    item.kind === 'link' && item.preview?.imageUrl
      ? { uri: item.preview.imageUrl }
      : item.kind === 'image' && bearer
        ? { uri: `${apiUrl()}/v1/stash/${encodeURIComponent(item.id)}/file`, headers: { authorization: `Bearer ${bearer}` } }
        : undefined;
  return (
    <View style={[styles.face, { width: size, height: size, borderRadius: size / 5 }, picture && styles.pictureFace]}>
      {picture ? (
        <Image source={picture} style={StyleSheet.absoluteFill} resizeMode="cover" resizeMethod="resize" fadeDuration={0} />
      ) : (
        <Icon name={KIND_ICONS[item.kind]} size={Math.round(size * 0.39)} color={colors.secondary} />
      )}
    </View>
  );
}

function Row({
  item,
  bearer,
  order,
  onPress,
  onLongPress,
}: {
  item: StashItem;
  bearer: string | undefined;
  order: number;
  onPress: () => void;
  onLongPress: () => void;
}) {
  const styles = useStyles(makeStyles);
  const file = item.kind === 'file' || item.kind === 'video';
  return (
    <Pressable
      onPress={onPress}
      onLongPress={onLongPress}
      style={[styles.row, order > 0 && styles.rowOn]}
      accessibilityRole="button"
      accessibilityLabel={itemTitle(item)}
      accessibilityHint="Hold to remove it from the stash"
      accessibilityState={{ selected: order > 0 }}>
      <Face item={item} bearer={bearer} />
      <View style={styles.body}>
        <Text style={[styles.rowTitle, file && styles.mono]} numberOfLines={1}>
          {itemTitle(item)}
        </Text>
        {item.note ? (
          <Text style={styles.note} numberOfLines={1}>
            {item.note}
          </Text>
        ) : null}
        <Text style={styles.place} numberOfLines={1}>
          {itemPlace(item)}
        </Text>
      </View>
      <View style={[styles.pick, order > 0 && styles.pickOn]}>{order > 0 ? <Text style={styles.pickText}>{order}</Text> : null}</View>
    </Pressable>
  );
}

const makeStyles = (colors: Palette) =>
  StyleSheet.create({
    wrap: { flex: 1 },
    head: { flexDirection: 'row', alignItems: 'center', height: 46, marginBottom: 6 },
    close: { width: 40, height: 40, alignItems: 'center', justifyContent: 'center' },
    title: { flex: 1, textAlign: 'center', fontFamily: fonts.uiSemibold, fontSize: 16, color: colors.ink },
    list: { flex: 1, marginHorizontal: -4 },
    listPad: { gap: 2 },
    row: { flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 62, padding: 8, borderRadius: 10 },
    rowOn: { backgroundColor: colors.active },
    face: {
      width: 46,
      height: 46,
      borderRadius: 9,
      borderWidth: 1,
      borderColor: colors.border,
      backgroundColor: colors.raised,
      alignItems: 'center',
      justifyContent: 'center',
      overflow: 'hidden',
    },
    pictureFace: { borderColor: 'transparent' },
    ghostFace: { borderColor: 'transparent' },
    ghostLine: { height: 11, borderRadius: 4, backgroundColor: colors.raised },
    body: { flex: 1, minWidth: 0 },
    rowTitle: { fontFamily: fonts.uiMedium, fontSize: 14.5, color: colors.ink },
    mono: { fontFamily: fonts.mono, fontSize: 13 },
    note: { marginTop: 3, fontFamily: fonts.ui, fontSize: 12.5, color: colors.secondary },
    place: { marginTop: 2, fontFamily: fonts.ui, fontSize: 12, color: colors.tertiary },
    pick: {
      width: 22,
      height: 22,
      borderRadius: 11,
      borderWidth: 1.5,
      borderColor: colors.borderStrong,
      alignItems: 'center',
      justifyContent: 'center',
    },
    pickOn: { borderColor: colors.accent, backgroundColor: colors.accent },
    pickText: { fontFamily: fonts.uiSemibold, fontSize: 12, color: colors.ground },
    empty: { alignItems: 'center', gap: 8, paddingTop: 70, paddingHorizontal: 36 },
    emptyTitle: { marginTop: 8, fontFamily: fonts.uiSemibold, fontSize: 16, color: colors.ink },
    emptyText: { fontFamily: fonts.ui, fontSize: 14, lineHeight: 20, color: colors.tertiary, textAlign: 'center' },
  });
